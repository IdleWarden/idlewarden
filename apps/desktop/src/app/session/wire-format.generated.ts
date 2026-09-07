import type {
  Command,
  Refusal,
  Session,
  SessionEvent,
  SignalValue,
} from "./session.model";

export const COMMANDS: readonly Command[] = [
  {
    "command": "start",
    "plugin": "dev.idlewarden.example-game",
    "profile": "default"
  },
  {
    "command": "stop"
  },
  {
    "command": "pause"
  },
  {
    "command": "resume"
  },
  {
    "command": "set_dry_run",
    "enabled": false
  },
];

export const EVENTS: readonly SessionEvent[] = [
  {
    "event": "game_detected",
    "plugin": "dev.idlewarden.example-game",
    "window_title": "Example Game"
  },
  {
    "event": "game_lost"
  },
  {
    "event": "plugin_loaded",
    "plugin": "dev.idlewarden.example-game",
    "version": "26.9.5"
  },
  {
    "event": "plugin_failed",
    "plugin": "dev.idlewarden.broken",
    "reason": "rules.json is not valid JSON"
  },
  {
    "event": "observed",
    "observation": {
      "frame_id": 42,
      "captured_at_ms": 1250,
      "signals": [
        {
          "id": "ui.screen_id",
          "value": {
            "type": "enum",
            "value": "main"
          },
          "confidence": 0.93
        },
        {
          "id": "progress.bar",
          "value": {
            "type": "ratio",
            "value": 0.5
          },
          "confidence": 1.0
        }
      ]
    }
  },
  {
    "event": "intent_proposed",
    "intent": {
      "name": "collect_reward",
      "params": {
        "slot": {
          "type": "int",
          "value": 3
        }
      }
    }
  },
  {
    "event": "intent_rejected",
    "intent": {
      "name": "collect_reward",
      "params": {
        "slot": {
          "type": "int",
          "value": 3
        }
      }
    },
    "reason": "rate ceiling reached"
  },
  {
    "event": "action_started",
    "intent": {
      "name": "collect_reward",
      "params": {
        "slot": {
          "type": "int",
          "value": 3
        }
      }
    }
  },
  {
    "event": "action_finished",
    "intent": {
      "name": "collect_reward",
      "params": {
        "slot": {
          "type": "int",
          "value": 3
        }
      }
    },
    "outcome": {
      "outcome": "succeeded"
    }
  },
  {
    "event": "action_finished",
    "intent": {
      "name": "collect_reward",
      "params": {
        "slot": {
          "type": "int",
          "value": 3
        }
      }
    },
    "outcome": {
      "outcome": "failed",
      "reason": "the post-condition did not hold"
    }
  },
  {
    "event": "action_finished",
    "intent": {
      "name": "collect_reward",
      "params": {
        "slot": {
          "type": "int",
          "value": 3
        }
      }
    },
    "outcome": {
      "outcome": "rejected",
      "reason": "the window lost focus"
    }
  },
  {
    "event": "action_finished",
    "intent": {
      "name": "collect_reward",
      "params": {
        "slot": {
          "type": "int",
          "value": 3
        }
      }
    },
    "outcome": {
      "outcome": "aborted"
    }
  },
  {
    "event": "action_finished",
    "intent": {
      "name": "collect_reward",
      "params": {
        "slot": {
          "type": "int",
          "value": 3
        }
      }
    },
    "outcome": {
      "outcome": "timed_out",
      "after_ms": 3000
    }
  },
  {
    "event": "agent_paused",
    "reason": "confidence dropped below the floor"
  },
  {
    "event": "agent_resumed"
  },
  {
    "event": "kill_switch"
  },
  {
    "event": "error",
    "message": "capture backend unavailable"
  },
];

export const SESSIONS: readonly Session[] = [
  {
    "state": "searching",
    "dry_run": true,
    "actions_taken": 0,
    "last_reason": null,
    "plugin": null,
    "profile": null
  },
  {
    "state": "ready",
    "dry_run": true,
    "actions_taken": 0,
    "last_reason": null,
    "plugin": null,
    "profile": null
  },
  {
    "state": "running",
    "dry_run": false,
    "actions_taken": 12,
    "last_reason": null,
    "plugin": "dev.idlewarden.example-game",
    "profile": "default"
  },
  {
    "state": "paused",
    "dry_run": false,
    "actions_taken": 12,
    "last_reason": "confidence dropped below the floor",
    "plugin": "dev.idlewarden.example-game",
    "profile": "default"
  },
  {
    "state": "halted",
    "dry_run": true,
    "actions_taken": 0,
    "last_reason": null,
    "plugin": null,
    "profile": null
  },
];

export const REFUSALS: readonly Refusal[] = [
  {
    "refusal": "no_game_ready"
  },
  {
    "refusal": "halted"
  },
  {
    "refusal": "not_running"
  },
  {
    "refusal": "not_paused"
  },
  {
    "refusal": "running_dry_run_change"
  },
];

export const SIGNAL_VALUES: readonly SignalValue[] = [
  {
    "type": "bool",
    "value": true
  },
  {
    "type": "int",
    "value": -7
  },
  {
    "type": "float",
    "value": 1.5
  },
  {
    "type": "ratio",
    "value": 0.25
  },
  {
    "type": "text",
    "value": "gold"
  },
  {
    "type": "point",
    "value": {
      "x": 0.5,
      "y": 0.75
    }
  },
  {
    "type": "rect",
    "value": {
      "x": 0.1,
      "y": 0.2,
      "w": 0.3,
      "h": 0.4
    }
  },
  {
    "type": "enum",
    "value": "main"
  },
];
