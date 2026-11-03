---
title: Combat
description: Préparer des rencontres, gérer l'initiative, suivre PV et états, et déplacer les pions sur la carte de combat.
sidebar:
  order: 6
---

Ouvrez **Combat** dans la barre latérale de la campagne. L'ordre des tours est à gauche et la carte de combat à droite (dans une fenêtre étroite, la carte passe sous l'ordre des tours).

## Pour le MJ

### Rencontres

Le MJ peut préparer autant de rencontres qu'il veut : tapez un nom et cliquez sur **Nouvelle rencontre**, puis passez de l'une à l'autre avec la liste **Rencontre**. Les joueurs ne voient rien d'une rencontre tant que le MJ n'a pas cliqué sur **Lancer pour les joueurs**. Une seule rencontre se joue à la fois : en lancer une arrête les autres. **Arrêter** la retire aux joueurs, et **Supprimer la rencontre** demande une confirmation avant de la supprimer.

### Combattants

Sous **Ajouter des combattants** :

- **Ajouter le groupe** ajoute le personnage actif de chaque joueur, avec les PV et la CA de sa fiche.
- **Chercher des monstres** cherche dans les sources du compendium activées pour la campagne. **Ajouter** reprend les PV et la CA du bloc de statistiques et numérote les copies (« Gobelin 2 »).
- **Ajout rapide** prend un nom, des PV et une CA pour tout le reste.

Sur chaque ligne, tapez un montant puis cliquez sur **Dégâts** ou **Soigner**. Les PV temporaires partent en premier. **Modifier** donne accès aux états (choisis dans la liste ou tapés librement, avec un nombre de rounds facultatif), à la concentration, aux PV max, à la CA, à **Caché aux joueurs** et au retrait. Cliquez sur un état pour le retirer.

### Initiative et tours

Tapez une valeur d'initiative ou cliquez sur le dé à côté. Le jet passe par le journal des dés avec le bonus de la créature : Dextérité et bonus d'initiative de la fiche pour les personnages, initiative du bloc de statistiques pour les monstres. **Initiative des monstres** lance pour chaque combattant qui n'est pas un personnage et n'a pas encore d'initiative, en jet pour le MJ. La liste se trie d'elle-même par initiative.

**Commencer le combat** ouvre le round 1 avec le premier combattant. **Tour suivant** et **Tour précédent** parcourent l'ordre, et **Remettre à zéro** revient à avant le combat. Un état avec un nombre de rounds diminue à la fin du tour de sa créature et disparaît à zéro. Le tour en cours est marqué par ▶ et « son tour », pas seulement par une couleur.

### Ce que voient les joueurs

La règle de campagne choisie dans **Réglages** décide de ce que les joueurs reçoivent de l'ordre des tours :

- **Exact :** tout sauf les combattants cachés, PV et CA des monstres compris.
- **Vague :** les monstres affichent En forme, Blessé, Au bord de la mort ou Hors de combat au lieu de leurs PV et CA. Les personnages gardent leurs PV.
- **Masqué :** les joueurs ont la carte mais pas l'ordre des tours.

Le serveur applique cette règle avant d'envoyer quoi que ce soit : les valeurs cachées n'arrivent jamais dans l'application des joueurs.

### La carte

- **Mur** et **Zone** : choisissez l'outil, puis cliquez sur une case pour en poser un. L'outil reste actif jusqu'à ce que vous choisissiez **Sélection**.
- **Placer un pion pour…** liste les combattants sans pion. Choisissez-en un et cliquez sur une case. Le pion d'un personnage prend la couleur de son joueur et lui appartient.
- **Sélection** : choisissez une forme pour changer son étiquette, sa largeur, sa hauteur ou sa couleur, la cacher aux joueurs ou la retirer. Faites glisser n'importe quelle forme pour la déplacer.
- **Largeur** et **Hauteur** fixent la taille de la grille, jusqu'à 200 cases sur 200.
- **Verrouiller la carte pour les joueurs** empêche les joueurs de déplacer leurs pions. **Montrer les déplacements en direct** fait voir à tous un pion bouger pendant qu'on le fait glisser, pas seulement quand on le lâche.

Les formes cachées apparaissent pâles et en pointillés pour le MJ. Les joueurs ne les reçoivent jamais.

## Pour les joueurs

Quand le MJ lance une rencontre, vous voyez son ordre des tours, si la campagne le montre, et sa carte. Votre pion a un contour épais. Faites-le glisser pour le déplacer : par défaut les autres le voient quand vous le lâchez, ou pendant le glisser si le MJ a activé les déplacements en direct. Vous pouvez taper ou lancer l'initiative de votre personnage. Tout le reste appartient au MJ.

Votre pion porte les initiales de votre personnage, pour le retrouver sans compter sur la couleur. Utilisez **+** et **−** pour zoomer sur la carte.
