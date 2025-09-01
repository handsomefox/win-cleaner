package cleaner

import (
	"fmt"
)

func HumanBytes(b uint64) string {
	const (
		k = 1024
		m = k * 1024
		g = m * 1024
		t = g * 1024
	)
	switch {
	case b >= t:
		return fmt.Sprintf("%.2f TB", float64(b)/float64(t))
	case b >= g:
		return fmt.Sprintf("%.2f GB", float64(b)/float64(g))
	case b >= m:
		return fmt.Sprintf("%.2f MB", float64(b)/float64(m))
	case b >= k:
		return fmt.Sprintf("%.2f KB", float64(b)/float64(k))
	default:
		return fmt.Sprintf("%d B", b)
	}
}
