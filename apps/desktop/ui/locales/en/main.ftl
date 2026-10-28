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
notes-empty = No party notes yet. Notes shared with the whole party appear here.
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
err-bad-file = This file isn't a Beholder library file, or it's damaged.
err-storage = Couldn't save data on this device.
err-image-large = This image is too large (max 20 MB, 40 megapixels).
err-image-unsupported = Use a JPEG, PNG or WebP image.
reject-forbidden = You're not allowed to do that.
reject-not-found = That no longer exists, or you can't see it.
reject-invite = No campaign uses that invite code.
reject-invalid = The server refused an invalid value.
reject-image = The image wasn't uploaded. Try again.
reject-conflict = That change conflicted with another one.
reject-expired = A change waited too long offline and was dropped.
reject-stale = Someone else changed this in the meantime. Reload it and make your change again.

## Notes
notes-title = Notes
notes-search = Search notes
notes-filter-all = All
note-new = New note
note-untitled = Untitled note
notes-list-empty = No notes here yet.
notes-none-selected = Select a note, or create a new one.
note-gone = This note was deleted, or it's no longer shared with you.
note-by = by { $name }
chip-party = Party
chip-shared = Shared
chip-private = Private
chip-dm = DM only
chip-conflict = Conflict copy
note-saving = Saving…
note-saved = Saved
note-readonly = Read-only
note-readonly-hint = You can read this note, but only its audience can edit it.
note-dm-reading = You can read this private note because the campaign lets the DM read players' private notes.
note-edit = Edit
note-preview = Preview
note-insert-image = Insert image
note-uploading = Uploading…
note-reveal = Reveal to party
note-delete = Delete note
note-delete-confirm = Delete “{ $title }”? This can't be undone.
note-title-label = Note title
note-body-label = Note text
note-body-placeholder = Write in Markdown: # Heading, **bold**, *italic*, - list…
note-empty-body = This note is empty.
note-conflict-copy = This is a conflict copy: your edit clashed with someone else's. Merge what you need into the original, then delete this copy.
note-open-original = Open original
note-has-copy = Your last edit clashed with someone else's, so it was kept as a private conflict copy.
note-open-copy = Open copy
note-changed-remotely = Someone else changed this note. If you keep your edit, it will be saved as a separate copy.
audience-label = Who can see this
audience-private = Only me
audience-dm-only = DM only (just you)
audience-party = The whole party
audience-users = Or share with specific members:
audience-users-hint = Members you tick can read and edit this note.

## Compendium
kind-spell = Spell
kinds-spell = Spells
kind-monster = Monster
kinds-monster = Monsters
kind-magic-item = Magic item
kinds-magic-item = Magic items
kind-equipment = Equipment
kinds-equipment = Equipment
kind-class = Class
kinds-class = Classes
kind-subclass = Subclass
kinds-subclass = Subclasses
kind-species = Species
kinds-species = Species
kind-background = Background
kinds-background = Backgrounds
kind-feat = Feat
kinds-feat = Feats
kind-feature = Class feature
kinds-feature = Class features
kind-trait = Species trait
kinds-trait = Species traits
kind-condition = Condition
kinds-condition = Conditions
cmp-search = Search spells, monsters, items…
cmp-kind = Type
cmp-all-kinds = Everything
cmp-any = Any
cmp-level = Level
cmp-cantrips = Cantrips
cmp-school = School
cmp-class = Class
cmp-cr-min = CR from
cmp-cr-max = CR to
cmp-rarity = Rarity
cmp-cantrip = { $school } cantrip
cmp-spell-level = Level { $level } { $school }
cmp-monster = CR { $cr } · { $size } { $type }
cmp-attunement = attunement
cmp-subclass = { $class } subclass
cmp-feature = { $class } level { $level }
cmp-no-results = Nothing matches. Try fewer words or filters.
cmp-refine = Showing the first 300 results. Add words or filters to narrow it down.
cmp-none-selected = Search or pick an entry to read it here.
cmp-syncing = Downloading the rules (about 200 KB, only once)…
cmp-failed = Couldn't download the rules. Beholder will try again the next time it connects.
cmp-offline = The rules download the first time you're online. After that, the compendium works offline.
cmp-credits = Credits
cmp-credits-intro = The compendium includes content under open licences. The required notices follow.
cmp-upstream = Converted from { $upstream }.
rules-2014 = 2014 rules
rules-2024 = 2024 rules
rules-level-up = Level Up (A5E)
cmp-source-filter = Source
cmp-others = +{ $count } { $count ->
    [one] version
   *[other] versions
}
cmp-also-in = Also in:
cmp-no-sources = This campaign uses no rules sources. The DM can turn them on in the campaign settings.
cmp-licenses = Licence texts
sources-label = Rules sources
sources-hint = Choose which sources this campaign uses. Players only download what's turned on. Entries with the same name show once, from the highest-priority source, with the others listed under "Also in".
sources-readonly = The DM chooses which sources this campaign uses.
sources-loading = Loading the list of sources…
sources-offline = Connect to the server to see every available source. Turned on now:
sources-size = { $kb } KB

## Server library
lib-new = New entry
lib-import = Import
lib-export-json = Export JSON
lib-export-yaml = Export YAML
lib-exported = Saved to { $path }
lib-imported = Import done: { $added } added, { $replaced } replaced, { $skipped } skipped as duplicates, { $notyours } not yours to replace, { $invalid } invalid.
lib-invalid-title = Entries that weren't imported
lib-edit = Edit
lib-delete = Delete entry
lib-delete-confirm = Delete this entry from the server library? It disappears for everyone on this server. This can't be undone.
lib-hide = Hide in this campaign
lib-unhide = Show in this campaign
lib-hidden = Hidden
lib-hidden-hint = Hidden from the players of this campaign. Only you see it.
lib-from-book = From { $book }
lib-editor-new = New library entry
lib-editor-edit = Edit library entry
lib-editor-hint = Library entries are shared by everyone on this server. The fields below are what search and filters use; the text holds the full rules in Markdown.
lib-name = Name
lib-book = Book
lib-book-hint = Where it comes from, e.g. "Player's Handbook (2024)" or "Homebrew"
lib-rules = Rules version
lib-text = Text (Markdown)
lib-classes = Classes (comma-separated)
lib-ritual = Ritual
lib-concentration = Concentration
lib-cr = Challenge rating
lib-creature-type = Creature type
lib-size = Size
lib-category = Category
lib-attunement = Requires attunement
tmpl-monster =
    {"*"}Size Type, Alignment*

    {"*"}*AC** 12 · **HP** 11 (2d8 + 2) · **Speed** 30 ft.

    | STR | DEX | CON | INT | WIS | CHA |
    |---|---|---|---|---|---|
    | 10 (+0) | 10 (+0) | 10 (+0) | 10 (+0) | 10 (+0) | 10 (+0) |

    {"*"}*Senses** Passive Perception 10

    {"*"}*Languages** Common

    {"*"}*CR** 1/4 (XP 50)

    ## Actions

    {"*"}*Bite.** Melee Attack Roll: +4, reach 5 ft. Hit: 5 (1d6 + 2) Piercing damage.
tmpl-spell =
    {"*"}Level 1 Evocation*

    | | |
    |---|---|
    | Casting Time | Action |
    | Range | 60 feet |
    | Components | V, S |
    | Duration | Instantaneous |

    Describe what the spell does.

    {"*"}*Using a Higher-Level Spell Slot.** Describe what changes.
lib-reading = Reading the file…
lib-importing = Sending { $count } imported entries to the server…
lib-replace = Replace entries already in the library
lib-bulk = Import any JSON
bulk-title = Import a JSON file of any shape
bulk-hint = Pick a JSON file from another tool or website. Beholder finds the list of entries, suggests which keys hold the name, type, text, book and fields, and shows a preview. Adjust the mapping, check the whole file, then import. Save the mapping as a profile to reuse it for similar files.
bulk-pick = Choose a JSON file…
bulk-entries-at = entries at { $path }
bulk-entries-root = entries at the top level
bulk-sample = First entries in the file
bulk-profile = Mapping
bulk-profile-saved = Saved profiles
bulk-profile-suggested = Suggested from the file
bulk-profile-name = Profile name
bulk-profile-save = Save profile
bulk-profile-export = Export profile
bulk-profile-delete = Delete profile
bulk-profile-import = Import a profile…
bulk-profile-exported = Profile saved to { $path }
bulk-name-key = Name
bulk-text-keys = Text (keys separated by commas)
bulk-kind-key = Entry type
bulk-kinds = Types found in the file
bulk-default-kind = Type when unknown
bulk-book-key = Book
bulk-book-fixed = Book when missing
bulk-rules-from-book = Use 2024 rules when the book name says 2024 (and Level Up when it says Level Up)
bulk-fields = Structured fields
bulk-none = —
bulk-scan = Check the whole file
bulk-scanning = Reading the whole file…
bulk-summary = { $total } entries: { $duplicates } already in the library, { $partial } partly mapped (kept, with "Unknown" fields), { $invalid } unreadable.
bulk-by-kind = By type
bulk-by-book = By book
bulk-no-book = (no book)
bulk-import = Import { $count } entries
lib-undo-import = Undo this import
lib-undo = Undo
lib-undoing = Undoing the import… Entries edited since then are kept.
lib-imports-title = Recent imports
lib-imports-empty = No imports yet.
lib-imports-loading = Loading the list of imports…
lib-import-row = { $date } · { $name } · { $count ->
    [one] 1 entry
   *[other] { $count } entries
}
lib-import-undone = Undone ({ $kept } kept because they were edited since)
lib-paste = Paste
paste-title = Paste import
paste-hint = Paste one entry or a whole section copied from a book or page you own. Each entry starts with its name on its own line, followed by its header line (spell level, monster size and type, item type and rarity, feat category, or "Class: Subclass" as the name). Text that can't be read is kept as one entry of the kind chosen below.
paste-default-kind = Kind when it can't be detected
paste-text = Pasted text
paste-read = Read entries
paste-found = { $count ->
    [one] 1 entry found. Check it, then accept.
   *[other] { $count } entries found. Check them, then accept.
}
paste-check = Check the fields
paste-duplicate = Already in the library
paste-preview = Preview
paste-back = Back to the text
paste-accept-all = Accept all
paste-accept-selected = Accept selected ({ $count })

ab-str = STR
ab-dex = DEX
ab-con = CON
ab-int = INT
ab-wis = WIS
ab-cha = CHA
armor-unarmored = No armor (10 + DEX)
armor-armor = Armor (base + DEX, capped)
armor-defense = Unarmored Defense (10 + DEX + ability)
armor-natural = Natural armor (base + DEX)
armor-fixed = Fixed value
caster-none = Not a spellcaster
caster-full = Full caster
caster-half = Half caster
caster-half-up = Half caster (rounded up)
caster-third = Third caster
caster-pact = Pact magic
coin-cp = CP
coin-sp = SP
coin-ep = EP
coin-gp = GP
coin-pp = PP
party-character = { $name } ({ $classes }) · AC { $ac } · HP { $hp }/{ $max }
prof-none = —
prof-proficient = Proficient
prof-expertise = Expertise
sheet-abilities = Abilities and saving throws
sheet-ac = AC
sheet-ac-bonus = Other AC bonus
sheet-active = Active
sheet-add-class = Add a class
sheet-add-counter = Add a counter
sheet-add-feat = Add a feat
sheet-add-feature = Add a feature
sheet-add-item = Add an item
sheet-add-spell = Add a spell
sheet-armor = Armor
sheet-armor-base = Base AC
sheet-caster = Spellcasting
sheet-casting-line = { $class }: spell save DC { $dc }, spell attack { $attack }
sheet-classes = Classes
sheet-combat = Combat
sheet-conditions = Conditions (separated by commas)
sheet-counter-current = Current
sheet-counter-max = Maximum
sheet-counter-name = Resource
sheet-counter-new = New resource
sheet-counters = Resources (sorcery points, ki, rages…)
sheet-death-ko = Death save failures
sheet-death-ok = Death save successes
sheet-defense-plus = Plus ability
sheet-delete = Delete this character
sheet-dex-cap = Max DEX bonus
sheet-equipped = Equipped
sheet-exhaustion = Exhaustion
sheet-feats = Feats
sheet-features = Features
sheet-hit-dice = Hit dice spent (of { $total })
sheet-hit-die = Hit die
sheet-hp-current = Current HP
sheet-hp-max = Max HP
sheet-hp-temp = Temporary HP
sheet-initiative = Initiative
sheet-initiative-bonus = Other initiative bonus
sheet-inventory = Inventory and money
sheet-item = Item
sheet-item-note = Note
sheet-jack = Jack of All Trades (half proficiency to unproficient checks)
sheet-level = Level { $level } · proficiency { $pb }
sheet-linked = from the compendium
sheet-name = Character name
sheet-new = New character
sheet-new-name = Name of the new character
sheet-no-cap = no cap
sheet-none = No character selected. Create one, or pick one in the Characters tab of the campaign home.
sheet-notes = Notes
sheet-open = Open sheet
sheet-pact = Pact slots: { $count } of level { $level }, spent
sheet-passives = Passive Perception { $perception } · Insight { $insight } · Investigation { $investigation }
sheet-penalty = Exhaustion: { $penalty } to every d20 test.
sheet-prepared = Prepared
sheet-proficiency = Proficiency
sheet-quantity = Quantity
sheet-remove = Remove
sheet-save = Save
sheet-set-active = Set active
sheet-shield = Shield (+2)
sheet-skills = Skills
sheet-speed = Speed
sheet-spell-ability = Spellcasting ability
sheet-spells = Spells
sheet-use-average = Use average HP ({ $hp })
skill-acrobatics = Acrobatics
skill-animal-handling = Animal Handling
skill-arcana = Arcana
skill-athletics = Athletics
skill-deception = Deception
skill-history = History
skill-insight = Insight
skill-intimidation = Intimidation
skill-investigation = Investigation
skill-medicine = Medicine
skill-nature = Nature
skill-perception = Perception
skill-performance = Performance
skill-persuasion = Persuasion
skill-religion = Religion
skill-sleight-of-hand = Sleight of Hand
skill-stealth = Stealth
skill-survival = Survival
slot-1 = 1st-level slots spent (of { $total })
slot-2 = 2nd-level slots spent (of { $total })
slot-3 = 3rd-level slots spent (of { $total })
slot-4 = 4th-level slots spent (of { $total })
slot-5 = 5th-level slots spent (of { $total })
slot-6 = 6th-level slots spent (of { $total })
slot-7 = 7th-level slots spent (of { $total })
slot-8 = 8th-level slots spent (of { $total })
slot-9 = 9th-level slots spent (of { $total })
