package rpc

import (
	"net/http"

	"github.com/getcihub/cihub/agent/manager"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

// Server is a RPC handler that enables remote interaction
// between the runner orchestrator and an agent using the http transport.
type Server struct {
	manager manager.RunnerManager
	token   string
}

// NewServer returns a new rpc server that enables remote
// interaction with the runner orchestrator using the http transport.
func NewServer(manager manager.RunnerManager, token string) *Server {
	return &Server{
		manager: manager,
		token:   token,
	}
}

// Handler returns an http.Handler.
func (s Server) Handler() http.Handler {
	r := chi.NewRouter()

	r.Use(middleware.Recoverer)
	r.Use(middleware.NoCache)
	r.Use(authorization(s.token))

	r.Post("/ping", HandlePing())

	return r
}

func authorization(token string) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// prevents system administrators from accidentally
			// exposing an agent without credentials.
			if token == "" {
				w.WriteHeader(http.StatusForbidden)
			} else if token == r.Header.Get("X-CIHub-Token") {
				next.ServeHTTP(w, r)
			} else {
				w.WriteHeader(http.StatusUnauthorized)
			}
		})
	}
}
