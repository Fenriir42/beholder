---
title: Dice
description: Roll from your sheet or by hand, share rolls with the table, roll to the DM, and enter physical dice.
sidebar:
  order: 5
---

Every roll is made **on the server**, so nobody can tweak the result on their own computer, and the result reaches everyone who may see it at once. The **Dice log** in the campaign sidebar lists the last 50 rolls of the campaign, newest first.

## Rolling from the sheet

On your character sheet, almost every number rolls when you click it:

- **Saving throws and skills** in the left column, the **ability modifiers** at the top, and **Initiative**.
- In the **Actions** tab and the spell side panel: **To hit** for weapon and spell attacks, **Damage** and **Heal** for damage and healing dice. In the side panel, each slot level has its own buttons, so upcast damage is rolled correctly. Magic Missile's darts are rolled together.

Above the tabs, choose **Disadvantage**, **Normal** or **Advantage** before rolling a d20 (advantage rolls 2d20 and keeps the highest). Under the 2024 rules, your exhaustion penalty is added to every d20 roll automatically.

Your own result appears as a card under the vitals: the total, each die (dropped dice in brackets), and "Natural 20!" or "Natural 1". After a natural 20 on an attack, **Roll critical damage** rolls the damage with doubled dice. Other people's rolls only show in the Dice log, so they don't interrupt you.

## Rolling by hand

In the **Dice log**, type any expression and click **Roll**:

| You type | It rolls |
|---|---|
| `1d20+5` | one d20, plus 5 |
| `8d6` | eight d6 |
| `2d20kh1+3` | two d20, keep the highest, plus 3 |
| `4d6kh3` | four d6, keep the three highest |
| `d20 adv`, `d20+2 dis` | a d20 with advantage or disadvantage |
| `1d8+1d6-1` | several dice and a modifier |

Up to 100 dice of up to 1,000 sides, and 10 groups of dice. An optional "What for" text labels the roll in the log.

## Rolls to the DM

Tick **Roll to the DM only** (on the sheet or in the Dice log) and the roll is seen only by you and the DM. When the DM rolls this way, only the DM sees it: that's a secret DM roll. Other players never receive these rolls; the server keeps them out of their apps.

## Physical dice

Rolling real dice? Type the expression (for example `1d20+4`), type the results in the field next to it (`14`, or `3, 5` for several dice, in order), and click **Enter physical roll**. The log marks it as physical dice.
