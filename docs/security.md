# Security

This client is meant to run on machines you may not fully control (for example a
work computer). Here is exactly what it protects, and what it cannot.

## Your settings are encrypted at rest

Your server details and credentials are stored in
`%APPDATA%\TrustTunnel\settings.enc`, encrypted with a key derived from your
password (Argon2id) using authenticated encryption (XChaCha20-Poly1305).

- The **password is never stored** -- not even a hash. When you unlock, the app
  derives the key from what you typed and tries to decrypt; success proves the
  password was right (and that the file was not tampered with).
- Because the key lives only in your head, **copying the app and its files to
  another machine is useless** without the password.
- A strong Argon2id cost makes offline password guessing on a stolen file slow.

**Choose a real password.** It is the only thing standing between a copied
`settings.enc` and your credentials.

## What this defends against

- Someone copying the whole app/folder to another machine.
- Someone reading your files over the network / from a backup / from a disk image
  taken while the app is not connected.

## What it cannot defend against (be honest with yourself)

On a machine where an administrator is the adversary and is **actively** on the
box while you use the VPN, encryption cannot hide your credentials during use:
an admin can attach a debugger, dump memory, log keystrokes, or read the config
the engine is using. This is a fundamental limit of running any client on a
machine someone else controls -- no client-side encryption changes it.

The app minimizes the exposure it can:

- The plain-text engine config exists on disk only for about **1.5 seconds** at
  connect time (the engine parses it once, then it is shredded), and while it
  exists it is locked down to SYSTEM and Administrators.
- Nothing sensitive is written in plain text between sessions.

If your threat model is "the admins must never see that I use a VPN or its
credentials," the only real answer is not to run it on their machine (tunnel the
PC through a device you control instead).

## Kill switch

With **Kill switch** enabled, if the tunnel drops while you want to be
connected, the app blocks non-tunnel traffic (via the Windows Filtering
Platform) so your real IP does not leak while it reconnects. It is engaged only
while the tunnel is down and lifted once you are connected again. The filters are
tied to the running app, so if the app exits they are removed automatically --
you cannot get permanently locked off the network.

There is a small residual window (a couple of seconds) between the engine dying
and the block engaging.

## If you forget your password

There is no recovery -- that is the point. Delete
`%APPDATA%\TrustTunnel\settings.enc`, start the app, set a new password, and
re-enter (or re-import) your server details.
