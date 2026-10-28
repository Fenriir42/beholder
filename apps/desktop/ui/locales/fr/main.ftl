app-name = Beholder

## First launch
server-title = Connexion à votre serveur
server-hint = Saisissez l'adresse fournie par votre MJ.
server-url = Adresse du serveur
continue = Continuer
change-server = Changer de serveur

## Auth
auth-login = Se connecter
auth-register = Créer un compte
username = Nom d'utilisateur
display-name = Nom affiché
password = Mot de passe
auth-discord = Continuer avec Discord
auth-discord-soon = Disponible dans une prochaine version

## Top bar and connection
conn-online = En ligne
conn-connecting = Connexion
conn-reconnecting = Reconnexion ({ $queued } en attente)
conn-offline = Hors ligne (cache)
banner-reconnecting = Reconnexion… Vos modifications sont en attente et seront envoyées automatiquement.
banner-offline = Vous êtes hors ligne. Vos modifications sont enregistrées sur cet appareil et seront envoyées au retour de la connexion.
retry = Réessayer
account = Compte
sign-out = Se déconnecter
language = Langue

## Library
library-title = Vos campagnes
search = Rechercher une campagne
join-with-code = Rejoindre avec un code
new-campaign = Nouvelle campagne
join-or-create = Rejoindre ou créer une campagne
invite-code = Code d'invitation
campaign-title = Titre de la campagne
cancel = Annuler
create = Créer
join = Rejoindre
no-campaigns = Aucune campagne pour l'instant. Créez-en une, ou rejoignez-en une avec le code de votre MJ.
pending = En attente du serveur
joining = { $count ->
    [one] Connexion à une campagne…
   *[other] Connexion à { $count } campagnes…
}

## Campaign shell
all-campaigns = Toutes les campagnes
nav-home = Accueil
nav-sheet = Fiche de personnage
nav-notes = Notes
nav-compendium = Compendium
nav-combat = Combat
nav-dice = Journal des dés
coming-soon = Bientôt disponible
meta-dm = MJ : { $name } · { $count ->
    [one] { $count } membre
   *[other] { $count } membres
}
meta-you-dm = Vous êtes le MJ · { $count ->
    [one] { $count } membre
   *[other] { $count } membres
}
tab-characters = Personnages
tab-notes = Notes de campagne
tab-party = Groupe
tab-settings = Paramètres
tab-rules = Règles de la campagne
characters-empty = Aucun personnage pour l'instant. Les fiches de personnage arrivent dans une prochaine version.
notes-empty = Aucune note de groupe pour l'instant. Les notes partagées avec tout le groupe apparaissent ici.
presence-online = En ligne
presence-offline = Hors ligne
dm-badge = MJ

## Settings
title-label = Titre
save = Enregistrer
cover = Image de couverture
cover-change = Changer la couverture
cover-uploading = Envoi…
invite-label = Code d'invitation
invite-regenerate = Générer un nouveau code
invite-hint = Partagez ce code avec vos joueurs. En générer un nouveau désactive l'ancien.
rule-dm-notes = Le MJ peut lire les notes privées des joueurs
rule-combat = Ce que les joueurs voient des monstres et PNJ en combat
combat-exact = Exact : tout, PV compris
combat-vague = Vague : En forme, Ensanglanté ou Presque mort
combat-hidden = Masqué : les joueurs ne voient pas le suivi de combat
rules-readonly = Seul le MJ peut modifier ces règles.
yes = Oui
no = Non

## Errors
err-length = Trop court ou trop long.
err-character = Contient un caractère non autorisé.
err-bad-url = Saisissez une adresse comme https://dnd.exemple.fr
err-username-taken = Ce nom d'utilisateur est déjà pris.
err-credentials = Nom d'utilisateur ou mot de passe incorrect.
err-rate-limited = Trop de tentatives. Patientez une minute puis réessayez.
err-update = Mettez Beholder à jour : le serveur utilise une version plus récente.
err-network = Impossible de joindre le serveur. Vérifiez l'adresse et votre connexion.
err-server = Le serveur a renvoyé une réponse inattendue.
err-signed-out = Votre session a expiré. Reconnectez-vous.
err-bad-file = Ce fichier n'est pas un fichier de bibliothèque Beholder, ou il est endommagé.
err-storage = Impossible d'enregistrer les données sur cet appareil.
err-image-large = Cette image est trop lourde (20 Mo et 40 mégapixels maximum).
err-image-unsupported = Utilisez une image JPEG, PNG ou WebP.
reject-forbidden = Vous n'avez pas le droit de faire cela.
reject-not-found = Cet élément n'existe plus, ou vous n'y avez pas accès.
reject-invite = Aucune campagne n'utilise ce code d'invitation.
reject-invalid = Le serveur a refusé une valeur invalide.
reject-image = L'image n'a pas été envoyée. Réessayez.
reject-conflict = Cette modification est entrée en conflit avec une autre.
reject-stale = Quelqu'un d'autre l'a modifié entre-temps. Rechargez-le et refaites votre modification.
reject-expired = Une modification est restée trop longtemps hors ligne et a été abandonnée.

## Notes
notes-title = Notes
notes-search = Rechercher dans les notes
notes-filter-all = Toutes
note-new = Nouvelle note
note-untitled = Note sans titre
notes-list-empty = Aucune note ici pour l'instant.
notes-none-selected = Choisissez une note, ou créez-en une.
note-gone = Cette note a été supprimée, ou elle n'est plus partagée avec vous.
note-by = par { $name }
chip-party = Groupe
chip-shared = Partagée
chip-private = Privée
chip-dm = MJ uniquement
chip-conflict = Copie de conflit
note-saving = Enregistrement…
note-saved = Enregistrée
note-readonly = Lecture seule
note-readonly-hint = Vous pouvez lire cette note, mais seul son public peut la modifier.
note-dm-reading = Vous pouvez lire cette note privée car la campagne autorise le MJ à lire les notes privées des joueurs.
note-edit = Modifier
note-preview = Aperçu
note-insert-image = Insérer une image
note-uploading = Envoi…
note-reveal = Révéler au groupe
note-delete = Supprimer la note
note-delete-confirm = Supprimer « { $title } » ? Cette action est définitive.
note-title-label = Titre de la note
note-body-label = Texte de la note
note-body-placeholder = Écrivez en Markdown : # Titre, **gras**, *italique*, - liste…
note-empty-body = Cette note est vide.
note-conflict-copy = Ceci est une copie de conflit : votre modification est entrée en conflit avec celle de quelqu'un d'autre. Reportez ce qu'il faut dans l'originale, puis supprimez cette copie.
note-open-original = Ouvrir l'originale
note-has-copy = Votre dernière modification est entrée en conflit avec celle de quelqu'un d'autre ; elle a été gardée dans une copie de conflit privée.
note-open-copy = Ouvrir la copie
note-changed-remotely = Quelqu'un d'autre a modifié cette note. Si vous gardez votre modification, elle sera enregistrée dans une copie séparée.
audience-label = Qui peut la voir
audience-private = Moi seul
audience-dm-only = MJ uniquement (vous seul)
audience-party = Tout le groupe
audience-users = Ou partager avec certains membres :
audience-users-hint = Les membres cochés peuvent lire et modifier cette note.

## Compendium
kind-spell = Sort
kinds-spell = Sorts
kind-monster = Monstre
kinds-monster = Monstres
kind-magic-item = Objet magique
kinds-magic-item = Objets magiques
kind-equipment = Équipement
kinds-equipment = Équipement
kind-class = Classe
kinds-class = Classes
kind-subclass = Sous-classe
kinds-subclass = Sous-classes
kind-species = Espèce
kinds-species = Espèces
kind-background = Historique
kinds-background = Historiques
kind-feat = Don
kinds-feat = Dons
kind-feature = Aptitude de classe
kinds-feature = Aptitudes de classe
kind-trait = Trait d'espèce
kinds-trait = Traits d'espèce
kind-condition = État
kinds-condition = États
cmp-search = Rechercher sorts, monstres, objets…
cmp-kind = Type
cmp-all-kinds = Tout
cmp-any = Tous
cmp-level = Niveau
cmp-cantrips = Sorts mineurs
cmp-school = École
cmp-class = Classe
cmp-cr-min = FP de
cmp-cr-max = FP à
cmp-rarity = Rareté
cmp-cantrip = Sort mineur ({ $school })
cmp-spell-level = Niveau { $level } ({ $school })
cmp-monster = FP { $cr } · { $type }, taille { $size }
cmp-attunement = harmonisation
cmp-subclass = Sous-classe de { $class }
cmp-feature = { $class }, niveau { $level }
cmp-no-results = Aucun résultat. Essayez moins de mots ou de filtres.
cmp-refine = Affichage des 300 premiers résultats. Ajoutez des mots ou des filtres pour affiner.
cmp-none-selected = Cherchez ou choisissez une entrée pour la lire ici.
cmp-syncing = Téléchargement des règles (environ 200 Ko, une seule fois)…
cmp-failed = Impossible de télécharger les règles. Beholder réessaiera à la prochaine connexion.
cmp-offline = Les règles se téléchargent la première fois que vous êtes en ligne. Ensuite, le compendium fonctionne hors ligne.
cmp-credits = Crédits
cmp-credits-intro = Le compendium contient du contenu sous licences libres. Voici les mentions requises.
cmp-upstream = Converti depuis { $upstream }.
rules-2014 = règles 2014
rules-2024 = règles 2024
rules-level-up = Level Up (A5E)
cmp-source-filter = Source
cmp-others = +{ $count } { $count ->
    [one] version
   *[other] versions
}
cmp-also-in = Aussi dans :
cmp-no-sources = Cette campagne n'utilise aucune source de règles. Le MJ peut les activer dans les réglages de la campagne.
cmp-licenses = Textes des licences
sources-label = Sources de règles
sources-hint = Choisissez les sources utilisées par cette campagne. Les joueurs ne téléchargent que celles activées. Les entrées de même nom n'apparaissent qu'une fois, depuis la source prioritaire, les autres étant listées sous « Aussi dans ».
sources-readonly = Le MJ choisit les sources utilisées par cette campagne.
sources-loading = Chargement de la liste des sources…
sources-offline = Connectez-vous au serveur pour voir toutes les sources disponibles. Actuellement activées :
sources-size = { $kb } Ko

## Server library
lib-new = Nouvelle entrée
lib-import = Importer
lib-export-json = Exporter en JSON
lib-export-yaml = Exporter en YAML
lib-exported = Enregistré dans { $path }
lib-imported = Import terminé : { $added } ajoutées, { $replaced } remplacées, { $skipped } ignorées car en double, { $notyours } impossibles à remplacer (pas les vôtres), { $invalid } invalides.
lib-invalid-title = Entrées non importées
lib-edit = Modifier
lib-delete = Supprimer l'entrée
lib-delete-confirm = Supprimer cette entrée de la bibliothèque du serveur ? Elle disparaît pour tous les utilisateurs du serveur. Cette action est définitive.
lib-hide = Masquer dans cette campagne
lib-unhide = Afficher dans cette campagne
lib-hidden = Masquée
lib-hidden-hint = Masquée aux joueurs de cette campagne. Vous seul la voyez.
lib-from-book = Tiré de { $book }
lib-editor-new = Nouvelle entrée de bibliothèque
lib-editor-edit = Modifier l'entrée de bibliothèque
lib-editor-hint = Les entrées de la bibliothèque sont partagées par tous les utilisateurs du serveur. Les champs ci-dessous servent à la recherche et aux filtres ; le texte contient les règles complètes en Markdown.
lib-name = Nom
lib-book = Livre
lib-book-hint = D'où elle vient, par ex. « Player's Handbook (2024) » ou « Création maison »
lib-rules = Version des règles
lib-text = Texte (Markdown)
lib-classes = Classes (séparées par des virgules)
lib-ritual = Rituel
lib-concentration = Concentration
lib-cr = Facteur de puissance
lib-creature-type = Type de créature
lib-size = Taille
lib-category = Catégorie
lib-attunement = Nécessite un lien
tmpl-monster =
    {"*"}Taille Type, alignement*

    {"*"}*CA** 12 · **PV** 11 (2d8 + 2) · **Vitesse** 9 m

    | FOR | DEX | CON | INT | SAG | CHA |
    |---|---|---|---|---|---|
    | 10 (+0) | 10 (+0) | 10 (+0) | 10 (+0) | 10 (+0) | 10 (+0) |

    {"*"}*Sens** Perception passive 10

    {"*"}*Langues** commun

    {"*"}*FP** 1/4 (PX 50)

    ## Actions

    {"*"}*Morsure.** Jet d'attaque au corps à corps : +4, allonge 1,50 m. Touché : 5 (1d6 + 2) dégâts perforants.
tmpl-spell =
    {"*"}Niveau 1, évocation*

    | | |
    |---|---|
    | Temps d'incantation | Action |
    | Portée | 18 mètres |
    | Composantes | V, S |
    | Durée | Instantanée |

    Décrivez l'effet du sort.

    {"*"}*Avec un emplacement de niveau supérieur.** Décrivez ce qui change.
lib-reading = Lecture du fichier…
lib-importing = Envoi de { $count } entrées importées au serveur…
lib-replace = Remplacer les entrées déjà dans la bibliothèque
lib-undo-import = Annuler cet import
lib-undo = Annuler
lib-undoing = Annulation de l'import… Les entrées modifiées depuis sont conservées.
lib-imports-title = Imports récents
lib-imports-empty = Aucun import pour l'instant.
lib-imports-loading = Chargement de la liste des imports…
lib-import-row = { $date } · { $name } · { $count ->
    [one] 1 entrée
   *[other] { $count } entrées
}
lib-import-undone = Annulé ({ $kept } conservées car modifiées depuis)
lib-paste = Coller
paste-title = Import par collage
paste-hint = Collez une entrée ou une section entière copiée d'un livre ou d'une page que vous possédez. Chaque entrée commence par son nom sur une ligne, suivi de sa ligne d'en-tête (niveau du sort, taille et type du monstre, type et rareté de l'objet, catégorie du don, ou « Classe: Sous-classe » comme nom). Le texte illisible est gardé comme une seule entrée du type choisi ci-dessous.
paste-default-kind = Type si non détecté
paste-text = Texte collé
paste-read = Lire les entrées
paste-found = { $count ->
    [one] 1 entrée trouvée. Vérifiez-la, puis acceptez.
   *[other] { $count } entrées trouvées. Vérifiez-les, puis acceptez.
}
paste-check = Vérifier les champs
paste-duplicate = Déjà dans la bibliothèque
paste-preview = Aperçu
paste-back = Retour au texte
paste-accept-all = Tout accepter
paste-accept-selected = Accepter la sélection ({ $count })
