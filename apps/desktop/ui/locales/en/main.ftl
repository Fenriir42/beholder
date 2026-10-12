app-name = Beholder

## First launch
server-title = Connect to your server
server-hint = Enter the address your DM gave you.
server-url = Server address
continue = Continue
change-server = Change server

## Auth
auth-login = Log in
auth-register = Create account
username = Username
display-name = Display name
password = Password
auth-discord = Continue with Discord
auth-discord-soon = Coming in a later update

## Top bar and connection
conn-online = Online
conn-connecting = Connecting
conn-reconnecting = Reconnecting ({ $queued } queued)
conn-offline = Offline (cached)
banner-reconnecting = Reconnecting… Your changes are queued and will be sent automatically.
banner-offline = You're offline. Changes are saved on this device and sent when the connection is back.
retry = Retry now
account = Account
sign-out = Sign out
language = Language

## Library
library-title = Your campaigns
search = Search campaigns
join-with-code = Join with code
new-campaign = New campaign
join-or-create = Join or create a campaign
invite-code = Invite code
campaign-title = Campaign title
cancel = Cancel
create = Create
join = Join
no-campaigns = No campaigns yet. Create one, or join with the code your DM gave you.
pending = Waiting for the server
joining = { $count ->
    [one] Joining a campaign…
   *[other] Joining { $count } campaigns…
}

## Campaign shell
all-campaigns = All campaigns
nav-home = Home
nav-sheet = Character sheet
nav-notes = Notes
nav-compendium = Compendium
nav-combat = Combat
nav-dice = Dice log
coming-soon = Coming soon
meta-dm = DM: { $name } · { $count ->
    [one] { $count } member
   *[other] { $count } members
}
meta-you-dm = You are the DM · { $count ->
    [one] { $count } member
   *[other] { $count } members
}
tab-characters = Characters
tab-notes = Campaign notes
tab-party = Party
tab-settings = Settings
tab-rules = Campaign rules
characters-empty = No characters yet. Character sheets arrive in a later update.
notes-empty = No party notes yet. Notes and handouts arrive in a later update.
presence-online = Online
presence-offline = Offline
dm-badge = DM

## Settings
title-label = Title
save = Save
cover = Cover image
cover-change = Change cover
cover-uploading = Uploading…
invite-label = Invite code
invite-regenerate = Regenerate code
invite-hint = Share this code with your players. Regenerating it stops the old code from working.
rule-dm-notes = The DM can read players' private notes
rule-combat = What players see of monsters and NPCs in combat
combat-exact = Exact: everything, including HP
combat-vague = Vague: Healthy, Bloodied or Near death
combat-hidden = Hidden: players don't see the tracker
rules-readonly = Only the DM can change these rules.
yes = Yes
no = No

## Errors
err-length = This is too short or too long.
err-character = This contains a character that isn't allowed.
err-bad-url = Enter an address like https://dnd.example.org
err-username-taken = That username is taken.
err-credentials = Wrong username or password.
err-rate-limited = Too many attempts. Wait a minute and try again.
err-update = Please update Beholder: the server uses a newer version.
err-network = Can't reach the server. Check the address and your connection.
err-server = The server returned an unexpected answer.
err-signed-out = Your session ended. Please log in again.
err-storage = Couldn't save data on this device.
err-image-large = This image is too large (max 20 MB, 40 megapixels).
err-image-unsupported = Use a JPEG, PNG or WebP image.
reject-forbidden = Only the DM can do that.
reject-not-found = That campaign no longer exists, or you're not a member.
reject-invite = No campaign uses that invite code.
reject-invalid = The server refused an invalid value.
reject-image = The image wasn't uploaded. Try again.
reject-conflict = That change conflicted with another one.
reject-expired = A change waited too long offline and was dropped.
