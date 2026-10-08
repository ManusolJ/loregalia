import { invoke } from "@tauri-apps/api/core";
import { RouterOutlet } from "@angular/router";
import { signal, Component } from "@angular/core";

@Component({
  imports: [RouterOutlet],
  selector: "app-root",
  templateUrl: "./app.html",
})
export class App {
  // * Placeholder code from project init
  greetingMessage = signal("");

  greet(event: SubmitEvent, name: string): void {
    event.preventDefault();

    invoke<string>("greet", { name }).then((text) => {
      this.greetingMessage.set(text);
    });
  }
}
