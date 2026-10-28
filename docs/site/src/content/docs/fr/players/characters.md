---
title: Fiches de personnage
description: Créer des personnages, remplir une fiche assistée, et voir ce que le reste du groupe en voit.
sidebar:
  order: 4
---

Chaque joueur peut avoir plusieurs personnages dans une campagne, dont un seul **actif** à la fois : le personnage actif est celui montré au groupe (et, plus tard, utilisé par le suivi de combat et les dés).

## Créer un personnage

Sur l'accueil de la campagne, ouvrez l'onglet **Personnages**. Tapez un nom, choisissez la version des règles (2024 ou 2014) et cliquez sur **Nouveau personnage** : la fiche s'ouvre. Votre premier personnage est actif ; utilisez **Rendre actif** pour passer à un autre. **Fiche de personnage** dans la barre latérale de la campagne ouvre toujours votre personnage actif.

## Jouer avec la fiche

La fiche s'ouvre en vue de jeu, organisée comme D&D Beyond :

- **En haut :** CA, initiative, vitesse, maîtrise et points de vie. Tapez une quantité et cliquez sur **Dégâts** (les PV temporaires partent d'abord), **Soins** ou **PV temporaires**. Les six modificateurs de caractéristique sont dessous, avec les états et l'épuisement quand il y en a.
- **À gauche :** jets de sauvegarde et compétences avec leurs bonus (○ sans maîtrise, ● maîtrise, ◆ expertise) et valeurs passives.
- **Onglets :**
  - **Actions :** vos objets équipés, les ressources à usage limité avec des boutons − et +, les dés de vie et les jets contre la mort.
  - **Sorts :** DD et attaque des sorts, emplacements en points à cliquer pour les dépenser (●) ou les récupérer (○), filtres par niveau et recherche. Chaque sort affiche son école, la concentration et le rituel quand il est dans le compendium, et le point devant indique s'il est préparé. **Ajouter des sorts** liste les sorts de vos classes jusqu'au plus haut niveau que vous pouvez lancer. Décochez le filtre pour tous les voir, ou ajoutez tel quel un sort absent du compendium.
  - **Inventaire :** argent, et objets avec quantité − et + et une case « équipé ». **Ajouter des objets** cherche parmi les objets magiques et l'équipement.
  - **Aptitudes et traits :** espèce, historique, classes et sous-classes, dons et aptitudes. **Ajouter aptitudes et dons** cherche dans le compendium.
  - **Notes :** texte libre.
- **À droite :** cliquer sur un sort, un objet, une aptitude, une espèce ou une classe ouvre sa fiche du compendium à côté de la feuille, avec **Préparer**/**Ne plus préparer** et **Retirer** quand c'est utile. Les noms saisis sans lien (par exemple depuis un fichier importé) sont retrouvés dans le compendium par leur nom. Sur les fenêtres étroites, le panneau glisse par-dessus la fiche.

### Effets calculés

Quand un sort ou une arme est dans le compendium, la fiche calcule ce qu'il fait pour votre personnage :

- **Sorts :** le bonus d'attaque (« Attaque +6 »), ou le jet de sauvegarde de la cible et votre DD (« Sauvegarde de DEX DD 14 », avec « moitié en cas de réussite » quand c'est le cas), et les dés de dégâts ou de soins au niveau du sort, par exemple « 2d10 fire » pour Fire Bolt au niveau 5 ou « Soigne 2d8+3 » pour Cure Wounds. Les sorts mineurs progressent avec votre niveau, Magic Missile et Eldritch Blast indiquent combien de projectiles vous obtenez, et le panneau latéral liste les dégâts à chaque niveau d'emplacement que vous pouvez utiliser.
- **Armes :** le bonus d'attaque et les dégâts avec votre modificateur (FOR, DEX pour les armes à distance, le meilleur des deux pour les armes de finesse), plus les dégâts à deux mains des armes polyvalentes. La maîtrise des armes n'est pas encore suivie : les attaques supposent que vous êtes maîtrisé.

L'onglet **Actions** regroupe vos armes équipées, sorts mineurs et sorts préparés avec ces valeurs, prêts pour les dés à venir. Les sorts dont les dégâts ne sont pas écrits en dés n'affichent que leur texte.

### Repos et incantation

- **Repos court** et **Repos long**, en haut, demandent une confirmation puis appliquent le repos pour vous. Un repos court rend les ressources réglées pour revenir après un repos court, et les emplacements de pacte de l'occultiste. Dépensez vous-même les dés de vie avec **Soins**. Un repos long rend vos PV, tous les emplacements de sorts, les ressources de repos court et long et vos dés de vie (tous en règles 2024, la moitié de votre niveau et au moins un en 2014), efface les jets contre la mort, réduit l'épuisement de un et met fin à la concentration. Le réglage « Revient après un » de chaque ressource se trouve dans **Modifier le personnage**.
- **Lancer**, à côté d'un sort, dépense un emplacement du plus petit niveau qui vous reste (ou un emplacement de pacte). Il est grisé quand aucun emplacement ne permet de le lancer. Les sorts mineurs sont gratuits. Dans le panneau latéral, vous pouvez choisir un emplacement plus élevé avant de lancer.
- **Concentration :** lancer un sort de concentration affiche « Concentration sur… » en haut, avec **Arrêter la concentration**. Si vous êtes déjà concentré sur un autre sort, **Lancer** ouvre le panneau latéral, où le bouton indique quel sort il interrompra.
- **Maîtrise des armes :** dans le panneau latéral d'une arme, décochez **Maîtrise de cette arme** et son bonus d'attaque n'inclut plus votre bonus de maîtrise.

## Modifier le personnage

**Modifier le personnage**, en haut, ouvre la vue de construction : nom et version des règles, espèce et historique, classes (niveau, dé de vie, type de lanceur, caractéristique d'incantation), caractéristiques et maîtrise des sauvegardes, maîtrise des compétences, armure et vitesse, PV maximum, ressources, dons et aptitudes, plus **Exporter** et **Supprimer**. Cliquez sur **Terminé** pour revenir à la vue de jeu. Chaque champ s'enregistre dès que vous le quittez, même hors ligne (les modifications partent quand la connexion revient).

L'application calcule :

- les modificateurs, les jets de sauvegarde et les compétences, avec maîtrise, **expertise** et **Touche-à-tout** ;
- la Perception, l'Intuition et l'Investigation passives, et l'initiative ;
- le bonus de maîtrise selon votre niveau total ;
- la **CA** selon la formule choisie : sans armure (10 + DEX), armure (base + DEX plafonnée), défense sans armure (10 + DEX + une autre caractéristique), armure naturelle ou valeur fixe, plus un bouclier et d'autres bonus ;
- le DD et l'attaque des sorts pour chaque classe qui a une caractéristique d'incantation ;
- les **emplacements de sorts** selon vos classes, avec la table du multiclassage et les emplacements de pacte de l'occultiste ;
- les PV maximum moyens selon vos dés de vie, applicables en un clic.

**Classes :** ajoutez une classe et choisissez-la dans le compendium. Pour les classes standard, le dé de vie, le type de lanceur de sorts et la caractéristique d'incantation sont remplis pour vous ; pour les autres, indiquez-les vous-même. Les types sont : lanceur complet, demi-lanceur (paladin, rôdeur), demi-lanceur arrondi au supérieur (artificier), tiers de lanceur (chevalier occulte, escroc arcanique) et magie de pacte (occultiste). En 2014, les demi-lanceurs ont des emplacements à partir du niveau 2, en 2024 dès le niveau 1.

**Espèce, historique, dons, aptitudes, sorts et objets** se choisissent dans le compendium de la campagne pendant la saisie (marqués « du compendium »), ou se tapent librement pour ce qui n'y est pas.

Vous suivez aussi les PV (actuels, maximum, temporaires), les dés de vie dépensés, les jets contre la mort, les états, l'épuisement (en 2024, chaque niveau donne −2 à chaque test de d20), les emplacements dépensés, des ressources comme les points de sorcellerie ou les rages, l'inventaire et l'argent, et des notes libres.

La montée de niveau est manuelle : changez un niveau de classe et tout ce qui précède est recalculé. L'application n'applique pas les aptitudes de classe pour vous.

## Export et import

**Exporter**, en bas de la fiche, enregistre le personnage dans votre dossier Téléchargements sous forme de fichier JSON `beholder-character` (version 1), comme sauvegarde ou pour l'emmener dans une autre campagne ou sur un autre serveur. **Importer un personnage…**, dans l'onglet Personnages, choisit un tel fichier et l'ajoute comme nouveau personnage à vous dans cette campagne.

## Qui voit quoi

- **Vous et le MJ** voyez et modifiez votre fiche complète. Le MJ peut tout corriger, par exemple pour appliquer des dégâts.
- **Les autres joueurs** ne voient qu'un résumé dans l'onglet **Groupe** : le nom, les classes et niveaux, la CA et les PV du personnage actif. Le reste de votre fiche (notes, inventaire, sorts…) n'arrive jamais dans leur application.
