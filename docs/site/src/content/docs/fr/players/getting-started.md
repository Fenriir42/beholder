---
title: Premiers pas
description: Installer Beholder, se connecter au serveur du groupe et créer son compte.
sidebar:
  order: 1
---

## Installer l'application

Beholder fonctionne sous **Windows** et **Linux**.

- **Windows :** lancez l'installateur `Beholder_x.y.z_x64-setup.exe`. Si votre PC n'a pas encore Microsoft WebView2, l'installateur le télécharge (la plupart des PC sous Windows 10 et 11 l'ont déjà).
- **Debian, Ubuntu, Mint :** installez le paquet `.deb` (`sudo apt install ./beholder_x.y.z_amd64.deb`).
- **Fedora, openSUSE :** installez le paquet `.rpm` (`sudo dnf install ./beholder-x.y.z-1.x86_64.rpm`).

Votre MJ, ou la personne qui gère le serveur, vous dira où trouver ces fichiers.

## Se connecter au serveur

Au premier lancement, Beholder demande l'**adresse du serveur**, par exemple `https://dnd.exemple.fr`, fournie par la personne qui héberge le serveur du groupe. Vous pouvez la saisir sans `https://`.

Beholder retient l'adresse. Pour la changer plus tard, utilisez **Changer de serveur** sur l'écran de connexion.

## Créer son compte

1. Choisissez **Créer un compte**.
2. Choisissez un **nom d'utilisateur** (3 à 32 lettres, chiffres, `_`, `-` ou `.`, qui sert à vous connecter) et un **nom affiché** (ce que voient les autres joueurs).
3. Choisissez un mot de passe d'au moins 8 caractères.

Le bouton **Continuer avec Discord** est visible mais désactivé : la connexion Discord arrive dans une prochaine version. La connexion par mot de passe restera toujours disponible, car Discord n'est pas accessible partout.

## Rester connecté

Beholder garde votre session dans le gestionnaire de mots de passe du système (Gestionnaire d'identification de Windows, ou GNOME Keyring / KWallet sous Linux), pour que vous n'ayez pas à vous reconnecter à chaque fois.

Si votre bureau Linux n'a pas de gestionnaire de mots de passe, Beholder fonctionne quand même mais oublie la session à la fermeture : il faudra vous reconnecter au lancement suivant.

## Langue

Le bouton **EN / FR** de la barre du haut bascule toute l'interface entre l'anglais et le français, instantanément. Beholder retient votre choix.
