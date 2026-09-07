import { Routes } from "@angular/router";

import { ActivityComponent } from "./activity/activity.component";
import { DetectComponent } from "./detect/detect.component";
import { SessionComponent } from "./session/session.component";

export const routes: Routes = [
  { path: "", component: SessionComponent },
  { path: "detection", component: DetectComponent },
  { path: "activite", component: ActivityComponent },
];
