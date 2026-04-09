package cleaner

import "errors"

var (
	ErrCancelled      = errors.New("cancelled")
	ErrGUIUnavailable = errors.New("gui unavailable")
)
