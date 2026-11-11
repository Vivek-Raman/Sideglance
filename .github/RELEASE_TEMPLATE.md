## Installing on macOS

Download the `.dmg` below, open it, and drag Sideglance into Applications.

Sideglance is not signed with an Apple Developer ID or notarized by Apple. macOS may say the app is "damaged" and suggest moving it to the Trash.

If you downloaded this copy from this repository's official GitHub release, open Terminal and run:

```sh
xattr -rd com.apple.quarantine /Applications/Sideglance.app
```

Then open Sideglance again. This removes the download quarantine for Sideglance only, allowing it to open without Apple's normal first-launch verification. Only do this for a copy you trust.
