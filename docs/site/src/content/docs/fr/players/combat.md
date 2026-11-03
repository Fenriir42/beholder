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

### Jouer un combattant

Cliquez sur le nom d'un combattant dans l'ordre des tours pour ouvrir son panneau à droite, ou cochez **Suivre le tour** pour que le panneau passe à celui dont c'est le tour. Tant que le panneau est ouvert, cliquer sur un pion lié sur la carte passe à ce combattant.

- **Monstre :** ses PV et sa CA, un bouton par attaque (**+N Toucher**, puis les dés de dégâts), les DD de sauvegarde (avec « moitié en cas de réussite » quand le texte le dit), et le bloc de statistiques complet en dessous. Après un 20 naturel, **Lancer les dégâts critiques** double les dés. Les jets de monstre sont pour le MJ par défaut ; décochez la case pour les montrer à la table.
- **Personnage :** la fiche de jeu du personnage : sauvegardes, compétences, attaques, sorts et emplacements. Vous pouvez déjà modifier toutes les fiches.
- **Combattant personnalisé :** PV, CA et une case pour taper n'importe quel jet.

### Cibles et dégâts

Quand vous lancez un sort ou attaquez, choisissez d'abord les **Cibles** (n'importe quel combattant de la rencontre en cours). Seul votre côté est lancé : un jet d'attaque par cible, puis **Lancer les dégâts** sur le résultat (ou **Lancer les dégâts critiques** après un 20 naturel), ou les dégâts une seule fois pour un sort à jet de sauvegarde, avec le DD dans l'intitulé. Les cibles lancent leurs propres sauvegardes.

Le MJ voit **Appliquer…** sur chaque jet de dégâts ou de soins, dans la carte du résultat et dans le journal des dés. Les cibles du jet sont déjà cochées ; choisissez **Entier**, **Moitié** ou **Double** pour chaque combattant (sauvegarde, résistance ou vulnérabilité), puis **Appliquer N dégâts** ou **Soigner N**. Rien n'est appliqué avant que le MJ clique. Les dégâts d'un personnage vont sur sa fiche, et l'ordre des tours affiche toujours les PV d'un personnage depuis sa fiche.

### Économie d'action

Chaque ligne affiche **Action**, **Action bonus** et **Réaction** (✓ quand elles sont utilisées) et la distance parcourue face à la vitesse de la créature. Elles se réinitialisent au début du tour de la créature : une réaction reste dépensée jusque-là. Déplacer le pion de la créature dont c'est le tour ajoute la distance automatiquement (1,5 m, soit 5 ft, par case, diagonales comprises) ; corrigez avec **−5** et **+5**. Les joueurs marquent leur personnage, le MJ marque n'importe qui, et lancer un sort ou attaquer depuis la fiche marque l'action correspondante selon le temps d'incantation.

### Ce que voient les joueurs

La règle de campagne choisie dans **Réglages** décide de ce que les joueurs reçoivent de l'ordre des tours :

- **Exact :** tout sauf les combattants cachés, PV et CA des monstres compris.
- **Vague :** les monstres affichent En forme, Blessé, Au bord de la mort ou Hors de combat au lieu de leurs PV et CA. Les personnages gardent leurs PV.
- **Masqué :** les joueurs ont la carte mais pas l'ordre des tours.

Le serveur applique cette règle avant d'envoyer quoi que ce soit : les valeurs cachées n'arrivent jamais dans l'application des joueurs.

### La carte

- **Mur**, **Zone** et **Danger**, plus les listes **Terrain** et **Objet** : choisissez-en un, puis cliquez sur une case pour le poser. L'outil reste actif jusqu'à ce que vous choisissiez **Sélection**.
  - Terrain : terrain difficile, eau, abri partiel, important ou total, visibilité réduite ou nulle. Chacun a son motif et son nom écrit dessus.
  - Danger : une zone rayée avec un panneau d'avertissement. Écrivez son effet dans la note, par exemple `DD 13 Dex, 2d6 feu` : le DD s'affiche en pastille et les dés deviennent des boutons de jet. Cachez-le et il devient un piège que vous seul voyez.
  - Objets : portes (fermée, ouverte ou verrouillée, avec **Ouvrir la porte** / **Fermer la porte** dans l'éditeur), coffres, leviers, statues et sources de lumière.
- **Gabarit de zone :** choisissez cercle, cône, ligne ou cube et cliquez sur la case d'origine. Réglez la taille en pieds et tournez cônes et lignes avec ⟲ et ⟳ (15° à la fois). Les cases touchées s'éclairent et le panneau liste les pions à l'intérieur. Une case compte quand son centre est dans la zone.
- **Placer un pion pour…** liste les combattants sans pion. Choisissez-en un et cliquez sur une case. Le pion d'un personnage prend la couleur de son joueur et lui appartient.
- **Sélection** : choisissez une forme pour changer son étiquette, sa largeur, sa hauteur ou sa couleur, la cacher aux joueurs ou la retirer. Faites glisser n'importe quelle forme pour la déplacer.
- **Largeur** et **Hauteur** fixent la taille de la grille, jusqu'à 200 cases sur 200.
- **Verrouiller la carte pour les joueurs** empêche les joueurs de déplacer leurs pions. **Montrer les déplacements en direct** fait voir à tous un pion bouger pendant qu'on le fait glisser, pas seulement quand on le lâche.

Les formes cachées apparaissent pâles et en pointillés pour le MJ. Les joueurs ne les reçoivent jamais.

Chaque forme peut porter une **note** que les joueurs lisent en cliquant dessus.

### Aperçu en tant que joueur

Dans **Voir comme**, choisissez un joueur pour voir la page de combat exactement comme le serveur la lui envoie : seulement la rencontre en cours, sans formes ni combattants cachés, avec la santé des monstres selon la règle de la campagne. L'aperçu est en lecture seule et demande une connexion. Choisissez **Moi (MJ)** pour revenir.

## Pour les joueurs

Quand le MJ lance une rencontre, vous voyez son ordre des tours, si la campagne le montre, et sa carte. Votre pion a un contour épais. Faites-le glisser pour le déplacer : par défaut les autres le voient quand vous le lâchez, ou pendant le glisser si le MJ a activé les déplacements en direct. Vous pouvez taper ou lancer l'initiative de votre personnage, et cliquer sur son nom pour ouvrir votre fiche à côté de la carte.

Cliquez sur n'importe quelle forme pour lire son nom et sa note, et lancer les dés de la note d'un danger. Pour viser un sort, choisissez une forme dans **Gabarit de zone**, cliquez sur la case de départ, puis réglez sa taille et sa direction. Vous pouvez déplacer, modifier et retirer vos propres gabarits (jusqu'à 10) tant que la carte n'est pas verrouillée. Tout le reste appartient au MJ.

Votre pion porte les initiales de votre personnage, pour le retrouver sans compter sur la couleur. Utilisez **+** et **−** pour zoomer sur la carte.
