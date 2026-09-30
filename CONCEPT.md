# Trading Town — Concept

A living market you can *watch*, slow enough to understand.

## The idea in one line
A small town of traders follows one real company's share price. Every day the
price is posted on the town notice board, the townsfolk read it, chat with their
neighbours, decide to buy or sell, and win or lose coins. You watch the story of
a market unfold — booms, herding, and crashes — instead of staring at a chart.

## The cast — five guilds, five houses
Each guild is a behavioural "type", lives in its own coloured house, and trades
differently:

| Guild | House | Personality |
|---|---|---|
| **Patient** | blue | Long memory. Ignores gossip, trades on its own slow read. |
| **Calm** | green | Steady. Mild follower of the mood. |
| **Bubbly** | orange | Excitable. Chases whatever is trending. |
| **Herders** | purple | Copies whoever is nearby, hard. |
| **Reckless** | red | Jumpy gamblers who make oversized bets. |

You compose the town: how many of each guild live there.

## A day in the town (the trading process, as a story)
1. **Morning** — the day's real share price appears on the **notice board** in the square.
2. **Chatter** — townsfolk read it, glance at neighbours, and talk. Copying a
   neighbour's opinion *is* herding.
3. **Decision** — each trader leans in to **buy** (green), backs away to **sell/short**
   (red), or holds.
4. **Evening** — the market moves for real; coins are won or lost. Faces show it:
   joy when up 😍, stress when it crashes 😣.
5. **Over weeks** — some guilds grow rich, others go broke. Bubbles build when
   everyone piles in the same way; crashes punish the herd the most.

## What's really happening under the story
This is an **agent-based model** (an extension of the 1996 Santa Fe Artificial
Stock Market):
- Each trader carries evolving rules-of-thumb that forecast tomorrow's move, and
  **learns** from being right or wrong (a genetic algorithm weeds out bad rules).
- Traders **copy** fit neighbours across a social network (herding).
- The price is **real and external** (a company's actual daily history) — the town
  reacts to it; it does not invent it. *(This is the "Path 3" design.)*

The numbers it produces: each guild's **wealth (P&L)**, the town's **sentiment**
(% leaning bullish), and a **herding index** (how one-sided the crowd is).

## Why it's interesting / where it goes next
It turns an abstract question — *"how does crowd psychology amplify a real market?"* —
into something you can see and play with. Make the town mostly herders and watch
bubbles balloon; make it mostly patient and watch it stay calm. Later we can plug
in more inputs (news, fundamentals, more guild behaviours) to enrich the town.

## How you use it
Pick a company → compose the town → press **Run** → watch, slowly. Speed is a dial,
defaulting slow so every rise and fall is legible.
