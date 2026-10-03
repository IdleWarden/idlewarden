import { Routes } from "@angular/router";

import { ActivityComponent } from "./activity/activity.component";
import { CatalogueComponent } from "./catalogue/catalogue.component";
import { DetectComponent } from "./detect/detect.component";
import { EditorComponent } from "./editor/editor.component";
import { InGameComponent } from "./in-game/in-game.component";
import { LogsComponent } from "./logs/logs.component";
import { ProfilesComponent } from "./profiles/profiles.component";
import { SessionComponent } from "./session/session.component";

export const routes: Routes = [
  { path: "", component: SessionComponent },
  { path: "detection", component: DetectComponent },
  { path: "activite", component: ActivityComponent },
  { path: "journaux", component: LogsComponent },
  { path: "profils", component: ProfilesComponent },
  { path: "catalogue", component: CatalogueComponent },
  { path: "editeur", component: EditorComponent },
  { path: "en-jeu", component: InGameComponent },
];
