<?php

declare(strict_types=1);

namespace Shop;

final class Cart
{
    /** @var array<string, int> */
    private array $lines = [];

    public function add(string $sku, int $qty = 1): self
    {
        $this->lines[$sku] = ($this->lines[$sku] ?? 0) + $qty;
        return $this;
    }

    public function total(callable $price): float
    {
        return array_sum(array_map(fn ($sku, $q) => $price($sku) * $q, array_keys($this->lines), $this->lines));
    }
}

$cart = (new Cart())->add('apple', 3)->add('pear')->add('apple');
$note = <<<EOT
    Heredoc keeps
      its indentation
    EOT;
$raw = <<<'NOW'
  nowdoc $not_interpolated
  NOW;
$x = 5;
$y = $x - -2; // no fusion
# hash comment
echo $cart->total(fn ($s) => $s === 'apple' ? 0.5 : 0.75), " ", $note, " ", $raw, " ", $y, "\n";
