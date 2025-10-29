package rpc

type pingRequest struct {
	Machine string
}

type requestRequest struct {
	Labels []string
}

type acceptRequest struct {
	Name    string
	Machine string
}

type registerRequest struct {
	Name string
}

type watchRequest struct {
	Name string
}

type watchResponse struct {
	Done bool
}

type startedRequest struct {
	RunnerID int64
}

type completedRequest struct {
	RunnerID int64
	Status   string
}
