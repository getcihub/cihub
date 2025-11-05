package client

type serverError struct {
	Status  int
	Message string
}

func (s *serverError) Error() string {
	return s.Message
}
