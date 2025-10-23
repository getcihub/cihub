package rpc

type pingRequest struct {
	Machine string
}

type requestRequest struct {
	Labels []string
}

type acceptRequest struct {
	JobID   int64
	Machine string
}

type registerRequest struct {
	Job int64
}

type watchRequest struct {
	RunnerID int64
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
