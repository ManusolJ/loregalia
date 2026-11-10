use std::io::{self, BufRead, BufReader, Write};

use interprocess::local_socket::{
    GenericNamespaced, Listener, ListenerOptions, Name, Stream, prelude::*,
};
use loregalia_core::storage::APP_FOLDER;

const FOCUS_REQUEST: &str = "focus";

pub enum Instance {
    Primary(Listener),
    Secondary,
}

pub fn instance_name(user: Option<&str>) -> String {
    let user: String = user
        .unwrap_or_default()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect();

    if user.is_empty() {
        format!("{APP_FOLDER}.instance")
    } else {
        format!("{APP_FOLDER}.{user}.instance")
    }
}

pub fn current_instance_name() -> String {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok();

    instance_name(user.as_deref())
}

fn socket_name(name: &str) -> io::Result<Name<'_>> {
    name.to_ns_name::<GenericNamespaced>()
}

pub fn claim(name: &str) -> io::Result<Instance> {
    if request_focus(name).is_ok() {
        return Ok(Instance::Secondary);
    }

    match ListenerOptions::new()
        .name(socket_name(name)?)
        .create_sync()
    {
        Ok(listener) => Ok(Instance::Primary(listener)),
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => Ok(Instance::Secondary),
        Err(error) => Err(error),
    }
}

fn request_focus(name: &str) -> io::Result<()> {
    let mut stream = Stream::connect(socket_name(name)?)?;
    stream.write_all(format!("{FOCUS_REQUEST}\n").as_bytes())?;
    stream.flush()
}

pub fn wait_for_focus_request(listener: &Listener) -> io::Result<bool> {
    let mut connection = BufReader::new(listener.accept()?);
    let mut line = String::new();
    connection.read_line(&mut line)?;

    Ok(line.trim() == FOCUS_REQUEST)
}

#[cfg(test)]
mod tests {
    use super::{Instance, claim, instance_name, wait_for_focus_request};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_name(label: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("loregalia-test-{label}-{}-{nanos}", std::process::id())
    }

    #[test]
    fn the_name_is_per_user_and_safe_for_a_pipe() {
        assert_eq!(instance_name(Some("Manu")), "loregalia.Manu.instance");
        assert_eq!(
            instance_name(Some("Ana María\\x")),
            "loregalia.AnaMarax.instance"
        );
        assert_eq!(instance_name(None), "loregalia.instance");
        assert_eq!(instance_name(Some("  ")), "loregalia.instance");
    }

    #[test]
    fn the_first_launch_is_primary() {
        let name = unique_name("first");

        assert!(matches!(claim(&name).unwrap(), Instance::Primary(_)));
    }

    #[test]
    fn a_second_launch_asks_the_first_to_take_focus() {
        let name = unique_name("second");
        let Instance::Primary(listener) = claim(&name).unwrap() else {
            panic!("the first launch should be primary");
        };
        let waiter = std::thread::spawn(move || wait_for_focus_request(&listener));

        let second = claim(&name).unwrap();

        assert!(matches!(second, Instance::Secondary));
        assert!(waiter.join().unwrap().unwrap());
    }

    #[test]
    fn a_closed_first_launch_lets_the_next_one_be_primary() {
        let name = unique_name("closed");
        let first = claim(&name).unwrap();
        assert!(matches!(first, Instance::Primary(_)));
        drop(first);

        assert!(matches!(claim(&name).unwrap(), Instance::Primary(_)));
    }
}
