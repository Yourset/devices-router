# Troubleshooting

## Check Versions First

Both PCs should ideally run the same version. The current mainline version is `v0.1.12`.

Check the version on the overview page. If the remote is older, connect it to the host and let it update.

## Remote Cannot Find Host

Symptoms:

- The remote stays disconnected.
- Logs repeatedly show discovery failure or local scan failure.

Checklist:

1. The host app is running in `Host mode`.
2. Both PCs are on the same LAN.
3. Windows Firewall allows TCP `8765`, TCP `8767`, and UDP `8766`.
4. VPN, proxy TUN, or virtual adapters are not interfering.
5. The remote has not been configured with a wrong manual host IP.

You can temporarily enter the host IP manually on the remote, for example:

```text
192.168.31.18
```

## Connected But Keyboard Does Not Arrive

Follow the log chain:

1. Click `Keyboard to remote` on the remote.
2. Remote should log `switch request sent`.
3. Host should log `remote requested keyboard to remote`.
4. After typing on the host, host should log `forwarded key`.
5. Remote should log `input key`.

Diagnosis:

- Remote has no `switch request sent`: no usable local connection channel.
- Remote sends, but host has no request log: control message did not reach the host.
- Host receives request, but no `forwarded key`: keyboard hook or target state issue.
- Host forwards, but remote does not input: injection failed or the target input field is not focused.

## Mouse Movement Does Not Auto Switch

Check:

- Both sides are on `v0.1.12` or newer.
- Mouse follow is enabled on the `Mouse follow` page.
- After moving the mouse on the remote, remote logs mouse activity reporting.
- Host logs remote mouse activity and switches the keyboard target.

If the remote does not report activity, its mouse detection thread is not working. If it reports activity but the host sees nothing, the connection channel is broken.

## Connection Status Is Wrong

Since `v0.1.11`, the app uses bidirectional heartbeat. After disconnecting, the status should turn disconnected quickly.

If the UI still says connected:

- Click `Clear logs` and watch new logs.
- Close the remote app and see whether the host logs heartbeat failure.
- Confirm both sides are on `v0.1.11` or newer.

## Update Fails

Check whether the host update port is listening:

```powershell
netstat -ano | Select-String ":8767"
```

Check the manifest:

```powershell
Get-Content "$env:LOCALAPPDATA\Devices Router\updates\manifest.json"
```

Common causes:

- Host app is not running.
- Firewall blocks TCP `8767`.
- Package filename, size, or sha256 does not match the manifest.
- The remote is on an old version and needs one manual restart.

## Input Still Goes To Host

The keyboard target is probably still host. Click `Keyboard to remote`, or press `Ctrl+Alt+2` on the host.

If host still receives input, the host has not entered remote target mode or the low-level keyboard hook is not working. Restart the host app and try again.

## IME and Special Keys

The current focus is stable English letters, numbers, common control keys, and basic shortcuts. Chinese IME composition, media keys, and gaming keyboard macros may need further work.
