# Lock screen

SlintLu locks the session with the `ext-session-lock-v1` protocol (supported by niri,
Hyprland, Sway, river, KDE Plasma, labwc, Wayfire …) and checks passwords with PAM.

## Writing one

Export a component inheriting `LockWindow`; one instance covers every screen while locked.

```slint
import { LockWindow, Session, Clock } from "@slintlu";

export component LockScreen inherits LockWindow {
    background: black;
    VerticalLayout {
        alignment: center;
        Text { text: Clock.time; font-size: 64px; color: white; horizontal-alignment: center; }
        password := TextInput {
            input-type: InputType.password;
            enabled: !Session.authenticating;
            accepted => { Session.authenticate(self.text); self.text = ""; }
        }
        Text { text: Session.message; color: red; }
    }
    changed width => { password.focus(); }
}
```

`Session` provides `locked`, `user`, `authenticating`, `message` (PAM's error text) and
`lock-before-sleep`. Without a `LockWindow` component SlintLu refuses to lock, so a missing lock
screen can never leave you stuck.

## Locking

- from the shell: `Session.lock()` or `Power.lock()`
- from scripts and keybindings: `slintlu msg call Session.lock`
- `loginctl lock-session`, which idle daemons such as `swayidle` use:
  `swayidle -w timeout 300 'loginctl lock-session'`
- automatically before suspend, unless `Session.lock-before-sleep` is false

`loginctl unlock-session` (an administrator action) also unlocks. SlintLu sets logind's
`LockedHint`, so `loginctl show-session` reports the lock.

## PAM

SlintLu authenticates with the PAM service `slintlu` if `/etc/pam.d/slintlu` exists, else
`login`. A dedicated service lets you tune it, e.g. to allow fingerprint unlock:

```
# /etc/pam.d/slintlu
auth include login
```

`SLINTLU_PAM_SERVICE` overrides the service name (the test suite uses a non-existent service,
which PAM denies, so tests never count as failed logins for `pam_faillock`).

## Safety

If the shell crashes while locked, the compositor keeps the session locked and shows its own
fallback (often a solid colour). Switch to a TTY, log in and restart `slintlu`, or unlock with
`loginctl unlock-session`.
