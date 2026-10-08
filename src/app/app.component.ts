import { invoke } from "@tauri-apps/api/core";
import { RouterOutlet } from "@angular/router";
import { signal, Component } from "@angular/core";

@Component({
  imports: [RouterOutlet],
  selector: "app-root",
  styleUrl: "./app.component.css",
  templateUrl: "./app.component.html",
})
export class AppComponent {
  greetingMessage = signal("");

  greet(event: SubmitEvent, name: string): void {
    event.preventDefault();

    invoke<string>("greet", { name }).then((text) => {
      this.greetingMessage.set(text);
    });
  }
}
