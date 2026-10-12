---
title: Bad connections and offline play
description: What the connection pill and banner mean, and what happens to your changes while offline.
sidebar:
  order: 3
---

Beholder is built for connections that come and go. Everything you do is saved on your computer first, then sent to the server when possible.

## The connection pill

The pill in the top bar (and at the bottom of the campaign sidebar) always shows a coloured dot **and** a word:

| Pill | Meaning |
|---|---|
| **Online** | Connected and up to date. |
| **Connecting** | First connection after starting the app. |
| **Reconnecting (n queued)** | The connection dropped; Beholder is retrying. `n` changes are waiting to be sent. |
| **Offline (cached)** | Several attempts failed. You're seeing the data saved on your computer; Beholder keeps retrying in the background. |

When you're not online, a banner under the top bar explains that your changes are queued. It doesn't block anything. Click **Retry now** to try reconnecting immediately (for example right after your VPN comes back).

## What happens to your changes

- When you rename a campaign, change a setting or create a campaign, you see the result **immediately**, even offline.
- The change is stored on your computer and sent when the connection returns. Closing the app doesn't lose it.
- When the server accepts it, nothing visible happens. It just stays.
- If the server **refuses** it (for example, a player trying a DM-only action, or a campaign that no longer exists), Beholder undoes it on your screen and shows a short message explaining why.
- A change that waited **more than 7 days** offline is dropped, because the server can no longer guarantee it would be applied only once. You'll see a message if that happens.

## Starting without a network

Beholder opens straight onto your library from what it saved last time, without waiting for the server. Cover images you've already seen are kept on your computer too.

## Session expired

If you haven't opened Beholder for 30 days, or you logged out on purpose, the app asks you to log in again. Your saved data for that account stays on your computer.
