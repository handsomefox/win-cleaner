package gui

import (
	"image/color"
	"testing"
)

func TestClamp01(t *testing.T) {
	for _, tc := range []struct{ in, want float64 }{
		{-1, 0}, {0, 0}, {0.5, 0.5}, {1, 1}, {2, 1},
	} {
		if got := clamp01(tc.in); got != tc.want {
			t.Fatalf("clamp01(%v) = %v, want %v", tc.in, got, tc.want)
		}
	}
}

func TestMagnitudeColor(t *testing.T) {
	nrgba := func(c color.Color) color.NRGBA {
		n, ok := c.(color.NRGBA)
		if !ok {
			t.Fatalf("magnitudeColor returned %T, want color.NRGBA", c)
		}
		return n
	}

	// Zero bytes is muted (the "Not found" tone), not part of the heat scale.
	if got := nrgba(magnitudeColor(0)); got != colMuted {
		t.Fatalf("magnitudeColor(0) = %v, want colMuted %v", got, colMuted)
	}
	// At/below the low end (~1 MiB) the size reads as near-white text.
	if got := nrgba(magnitudeColor(1 << 20)); got != colText {
		t.Fatalf("magnitudeColor(1 MiB) = %v, want colText %v", got, colText)
	}
	// At/above the high end (~8 GiB) it saturates to red and clamps beyond.
	wantRed := color.NRGBA{R: 0xff, G: 0x5c, B: 0x5c, A: 0xff}
	if got := nrgba(magnitudeColor(1 << 33)); got != wantRed {
		t.Fatalf("magnitudeColor(8 GiB) = %v, want %v", got, wantRed)
	}
	if got := nrgba(magnitudeColor(1 << 40)); got != wantRed {
		t.Fatalf("magnitudeColor(1 TiB) = %v, want clamped red %v", got, wantRed)
	}
}
