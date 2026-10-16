# calcite-update

Signed, delta-capable app updates for Calcite apps on macOS, Windows, and Linux. The root
[README](../../README.md#updates) shows how to publish a release. This file states the
security model.

## How a client trusts an update

1. The client loads `update.json` over HTTPS (connect timeout 10 s, response timeout 60 s).
2. The envelope names the signing key (`key_id`). The client looks the key up in its trusted
   keys. An unknown `key_id` is an error before any parsing.
3. The client verifies the Ed25519 signature (`verify_strict`) over the exact feed text. Only
   then does it parse the feed.
4. The client checks the format, the product, the channel, and the freshness fields.
5. It downloads the package, checks size and SHA-256 against the signed feed, and stages the
   new tree. Every staged file must match the signed tree.
6. At install, it hashes the staged tree again. Then it installs the tree.

## Threats covered

| Threat                                        | Defense                                                                                                  |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| A changed feed or package                     | The signature covers the feed. The feed holds a SHA-256 for every package and file.                      |
| A changed staged file                         | The staged tree is hashed again right before the install.                                                |
| Replay of an older signed feed (rollback)     | The feed has a signed `sequence`. The client stores the highest value in `update-state.json` and rejects a lower one. |
| A frozen client (an old feed served forever)  | The feed has a signed `issued_at`. The client rejects a feed older than its maximum age (default 30 days) or more than 5 minutes in the future. |
| A leaked or retired key                       | The app trusts several keys by `key_id`. Ship the new key, sign with it, then drop the old key.           |
| An update that deletes the user's files       | An install writes `.calcite-install.json` (app ID, version, owned paths). It moves or deletes only paths in the old marker or the new tree. |
| A crash during an install                     | A per-file install writes a journal first. A failure restores every file. `cleanup` restores after a crash. |
| A link or path trick                          | Every manifest path is checked. Downloads use `create_new` in a folder that is not a link. Installed files open without following links. |
| A local `file://` or plain HTTP feed          | Only HTTPS loads. `file://` needs the `test-support` feature, which an app must not enable.              |

## Not covered

- A compromised signing key. Keep the seed secret, and rotate by `key_id` if it leaks.
- Operating system code signing and notarization. The bundle signature comes from
  `calcite-bundle`. An update replaces files after the OS checked the first install.
- An attacker with write access to the installed folder or the state folder.
- A client that never contacts the server. The age limit applies only when it does.

## Rules for the publisher

- Raise `sequence` with every signed feed.
- Sign a new feed at least once per maximum age, even without a new release.
- Keep the seed out of the repository (`CALCITE_UPDATE_KEY` or standard input).
- Use `minimum_version` to mark older versions as required to update.
- Enable the `publish` feature only in the release tool, not in the app.

## Install ownership

| Root                          | Install                                                                                  |
| ----------------------------- | ---------------------------------------------------------------------------------------- |
| macOS `.app` bundle           | The bundle is owned. The install swaps it whole and writes no marker.                    |
| Folder with a valid marker    | A whole-folder swap only if the folder holds the marker and the paths it lists, and nothing else (not on Windows). |
| Other folder, or no marker    | The install moves the listed files one by one, through a journal, and writes the marker. |
| Single file (AppImage)        | The new file replaces the old file.                                                      |

## Update state

`update-state.json` in the folder given to `state_dir` holds `{ "highest_sequence": n }`.
The client writes it atomically (temporary file, sync, rename), and only after a feed passes
every check. Do not put the folder inside the installed tree.
