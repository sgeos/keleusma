# Chapter 1. What Keleusma Is, and What It Is Not

## Goal

By the end of this chapter you will know what kind of language Keleusma
is, what it is built to do, and what it deliberately leaves out. You will
not write any code in this chapter. This is the one chapter in the guide
that is pure orientation. It sets expectations so that nothing later is a
surprise.

## A score and an orchestra

Consider a musical score. The score is not the orchestra. It produces no
sound on its own. It is a precise and finite set of instructions, and the
orchestra is what carries those instructions out and fills the hall with
sound.

Keleusma is a language for writing the score. The orchestra is a
separate, larger program called the host. The host does the loud and
complicated work, whether that is producing sound, drawing a game screen,
or driving a motor. The Keleusma program sits inside the host and tells
it, precisely and predictably, what to do and when.

This is the first thing to understand. Keleusma is an embedded language.
It is not meant to be a whole application on its own. It is meant to be
the small, exact, trustworthy part inside a larger program. Throughout
the guide the larger program is called the host.

## Running on a steady beat

A piece of music has a pulse. The conductor brings the baton down, the
players play one beat, and then they wait for the next beat. The pulse
does not stop and does not stumble.

A Keleusma program works the same way. It does a small, bounded amount of
work, hands control back to the host, and waits to be called again. Each
turn is one beat. This guide calls one such turn a tick. An audio program
might run one tick per sixteenth note. A game might run one tick per
frame. The host decides the tempo. Keleusma fills in what happens on each
tick.

## What Keleusma does not have, and why

Keleusma leaves out several things that most programming languages
include. Every omission is deliberate.

- No unbounded loops. Every repetition in Keleusma has a count that is
  known before the loop begins. A repeat sign in sheet music tells the
  player how many bars to repeat. It never says "repeat for a while, and
  we shall see."
- No recursion. A Keleusma function may not call itself, directly or
  through a chain of other functions.
- No free-form input. A Keleusma program does not pause to wait for
  someone to type at a console. Input arrives in a structured form, from
  the host, at a tick boundary.

The reason for every one of these omissions is a single promise.
Keleusma guarantees, before a program is ever run, that each tick will
finish within a bounded amount of time and a bounded amount of memory.
The constructs left out are exactly the ones that could run forever or
consume memory without limit. A language cannot make the promise and also
keep those constructs, so Keleusma keeps the promise.

## The promise, stated plainly

Because of these limits, several things can be known about a Keleusma
program before it runs at all:

- it will not freeze,
- it will not exhaust memory unexpectedly,
- it will always keep its beat.

Part V of the guide explains how the language checks these properties.
For now the point is only that the limits are not arbitrary. They are the
price of the promise, and the promise is the reason Keleusma exists. A
musician who cannot promise to finish the bar in time is not given a seat
in the orchestra. Keleusma applies the same standard to a program.

## How this guide works

The guide uses music as its way in. Many ideas in programming already
exist in music under different names, and the guide names the musical
idea first, then the programming idea, then the precise Keleusma form.
You do not need to read musical notation, and you do not need to play an
instrument. If you listen to music, you hold enough intuition to follow
along.

Every chapter after this one develops one small program, runs it, and
shows its output. The programs are short on purpose. The goal is for you
to type each one, run it, and change it.

## What you now know

- Keleusma is a small language embedded inside a larger host program.
- A Keleusma program runs in bounded turns called ticks.
- The language omits unbounded loops, recursion, and free-form input, in
  exchange for a guarantee that every tick finishes within bounded time
  and memory.

The next chapter installs the Keleusma tool and runs a first program.
