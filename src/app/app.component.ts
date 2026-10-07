import { invoke } from "@tauri-apps/api/core";
import { RouterOutlet } from "@angular/router";
import { signal, Component } from "@angular/core";

@Component({
  selector: "app-root",
  imports: [RouterOutlet],
  templateUrl: "./app.component.html",
  styleUrl: "./app.component.css",
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
