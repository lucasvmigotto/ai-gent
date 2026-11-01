#!/usr/bin/env bash
# A tiny Node service that calls a third-party payments SDK; no tests, no docs.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
mkdir -p src
cat >package.json <<'JSON'
{ "name": "shopapi", "version": "1.0.0",
  "dependencies": { "express": "4.18.2", "stripe": "14.0.0", "amqplib": "0.10.3" } }
JSON
cat >src/app.js <<'JS'
const express = require('express');
const Stripe = require('stripe');
const amqp = require('amqplib');
const app = express();
const stripe = new Stripe(process.env.STRIPE_KEY);
app.post('/orders/:id/pay', async (req, res) => {
  const intent = await stripe.paymentIntents.create({ amount: req.body.amount, currency: 'usd' });
  const ch = await (await amqp.connect(process.env.AMQP_URL)).createChannel();
  ch.sendToQueue('order-paid', Buffer.from(req.params.id));
  res.json({ id: intent.id });
});
app.listen(3000);
JS
git add -A
git commit -q -m "chore: initial commit"
git switch -q -c dev
