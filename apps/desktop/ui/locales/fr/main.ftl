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
notes-empty = Aucune note de groupe pour l'instant. Les notes et documents arrivent dans une prochaine version.
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
err-storage = Impossible d'enregistrer les données sur cet appareil.
err-image-large = Cette image est trop lourde (20 Mo et 40 mégapixels maximum).
err-image-unsupported = Utilisez une image JPEG, PNG ou WebP.
reject-forbidden = Seul le MJ peut faire cela.
reject-not-found = Cette campagne n'existe plus, ou vous n'en êtes pas membre.
reject-invite = Aucune campagne n'utilise ce code d'invitation.
reject-invalid = Le serveur a refusé une valeur invalide.
reject-image = L'image n'a pas été envoyée. Réessayez.
reject-conflict = Cette modification est entrée en conflit avec une autre.
reject-expired = Une modification est restée trop longtemps hors ligne et a été abandonnée.
