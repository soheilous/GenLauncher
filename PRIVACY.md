# Privacy

GenLauncher runs on your computer and has no server of its own. It collects no analytics and sends nothing about you
anywhere, except what is described here.

**Mod index and messages.** The launcher downloads the public files in [`index/`](index/) from GitHub (the list of mods
and the project's announcements). GitHub sees an ordinary web request.

**Mods.** Mods are downloaded by you, from the pages of their authors, in the launcher's built-in browser or your own.
Those sites see you as any visitor.

**Signing in with Google (optional).** If you choose *Sign in with Google*, GenLauncher asks for two things:

- your email address, only to show which account is signed in;
- access to the *app data folder* in your Google Drive: a hidden folder that only GenLauncher can read, which does not
  appear in your Drive. The launcher stores copies of its configuration file there (your settings, enabled patches and
  the look of the launcher; no mod files, no file paths, no passwords). It keeps the newest 20.

The sign-in is kept on your computer (in GenLauncher's settings folder) so you stay signed in. Nothing goes through a
server of ours. *Sign out* in Settings removes it and revokes the permission; you can also remove it at
<https://myaccount.google.com/permissions>. Deleting GenLauncher's data from Drive is done in the same place, or by
deleting the backups in Settings first.

GenLauncher's use of information received from Google APIs adheres to the
[Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy), including the
Limited Use requirements.

**Questions.** Open an issue at <https://github.com/soheilous/GenLauncher/issues>.
