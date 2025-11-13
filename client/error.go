package client

import "errors"

// ErrMachineNotFound is returned when the agent's machine token is not valid
var ErrMachineNotFound = errors.New("machine not found or deleted")

type serverError struct {
	Status  int
	Message string
}

func (s *serverError) Error() string {
	return s.Message
}
