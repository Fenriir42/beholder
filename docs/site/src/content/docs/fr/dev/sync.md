---
title: Synchronisation et protocole
description: Comment les clients et le serveur restent cohérents sur des liaisons peu fiables.
sidebar:
  order: 3
---

La conception est consignée dans l'ADR-0003 (protocole réseau), l'ADR-0004 (synchro) et l'ADR-0007 (réseau côté client).

## Transport et trames

- Un WebSocket sur TLS (`wss://hôte/ws`) pour la synchro en direct ; du HTTPS simple pour l'authentification et les images.
- Chaque trame est `[flags: u8][corps postcard]`. Le bit 0 indique un corps compressé en zstd, seulement au-delà de 1 Ko et si la compression fait vraiment gagner de la place. Les trames sont limitées à 4 Mio, compressées ou non.
- Les corps HTTP sont aussi en postcard : pas de JSON sur le réseau.
- Une connexion inactive n'envoie qu'un ping toutes les 15 s (quelques centaines d'octets par minute). Aucun trafic pendant 35 s signifie que la connexion est morte.

## Poignée de main et rattrapage

```
client                                   serveur
  Hello { version, jeton, known: [(campagne, dernier_seq)] } ─▶
                    ◀─ HelloOk { utilisateur, campagnes, jeton_renouvelé? }
                    ◀─ par campagne : Events (ceux manqués) ou Snapshot
                    ◀─ par campagne : Presence
  (file d'envoi renvoyée dans l'ordre) ─▶
```

Un instantané (snapshot) remplace les événements quand le client ne connaît pas la campagne, est en retard sur le point de compaction, a plus de 1 000 événements de retard, annonce un `seq` que le serveur n'a pas, ou quand la plage manquée change la visibilité (public d'une note ou réglages de la campagne), car le rejeu est filtré avec les publics actuels.

## Journal d'événements

Chaque campagne a un journal en ajout seul, avec un `seq` strictement croissant. Une commande s'exécute dans **une seule transaction SQLite** : vérification de doublon, vérification des droits, ajout de l'événement et mise à jour de la projection (tables `campaigns`, `memberships` et `notes`). `Campaign::apply`, dans `beholder-core`, est l'unique machine à états, utilisée par les tests du serveur et par chaque cache client.

## Filtrage par destinataire

`permissions.rs` décide, par utilisateur (`Viewer { user, role }`), quels événements et quelles parties d'un instantané chaque membre reçoit. Exemple : `InviteRegenerated` ne va qu'au MJ, et une note ne va qu'à son public, donc les autres voient un trou dans les `seq`, ce qui est normal. Quand le public d'une note change, ceux qui gagnent l'accès reçoivent la note complète et ceux qui le perdent reçoivent une suppression. Un changement de réglages renvoie à chaque membre un instantané filtré. La diffusion n'encode qu'une fois par utilisateur. Voir l'ADR-0011.

## Commandes, déduplication et file d'envoi

- Le client écrit chaque commande dans sa **file d'envoi** (SQLite) *avant* de l'envoyer, avec un identifiant UUIDv7.
- Le serveur garde la réponse (`Ack` ou `Reject`) de chaque commande pendant 8 jours. Une commande renvoyée reçoit la réponse enregistrée et ne change rien.
- Une commande dont l'horodatage UUIDv7 a plus de 7 jours est rejetée comme `Expired`, car le serveur ne peut plus prouver qu'il ne l'a pas déjà vue.
- L'UI affiche **l'état confirmé + les commandes en attente**. Un `Reject` retire la commande et la vue est recalculée : c'est tout le mécanisme d'annulation.

## Garanties d'ordre

Tout accès à la base passe par une seule connexion (`Db::call`). Le commit d'une commande et sa diffusion se font dans le même appel, de même que le rattrapage et l'abonnement d'une nouvelle connexion. Un client ne peut donc ni manquer un événement ni le recevoir dans le désordre.

## Mémoire bornée

Chaque connexion a une file sortante de 256 trames. Un client trop lent pour la vider est déconnecté et rattrape son retard à la reconnexion, au lieu de faire grossir la mémoire du serveur.

## États de connexion (client)

`Connecting` → `Online` → en cas de perte `Reconnecting { queued }` → après 3 échecs `Offline { queued }`. Les tentatives espacées vont de 0,5 s à 10 s, avec une part aléatoire. `network_changed()` réessaie immédiatement.

## Les tests qui protègent tout ça

- `crates/protocol` : chaque message fait l'aller-retour ; les trames invalides et les bombes de décompression sont refusées.
- `crates/server/src/permissions.rs` : la matrice des permissions.
- `crates/server/tests/ws.rs` : sur de vraies sockets, un joueur ne reçoit jamais le code d'invitation, ni en direct ni au rattrapage ; une commande renvoyée n'est appliquée qu'une fois.
- `crates/e2e/tests/flaky_network.rs` : un proxy ajoute de la latence et coupe les connexions au hasard ; aucune commande n'est perdue ni dupliquée, et les deux clients convergent.
