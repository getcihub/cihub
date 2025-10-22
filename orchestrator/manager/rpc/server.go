package rpc

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"

	"github.com/getcihub/cihub/orchestrator/manager"
)

// Server wraps the chi Router in a custom type for wire
// injection purposes.
type Server http.Handler

// NewServer returns a new RPC server that enables remote interaction
// between a server and an agent using the http transport.
func NewServer(manager manager.RunnerManager, secret string) Server {
	r := chi.NewRouter()

	r.Use(middleware.Recoverer)
	r.Use(middleware.NoCache)
	r.Use(authorization(secret))

	r.Post("/ping", HandlePing())
	r.Post("/request", HandleRequest(manager))
	r.Post("/accept", HandleAccept(manager))
	r.Post("/details", HandleDetails(manager))
	r.Post("/watch", HandleWatch(manager))

	return Server(r)
}

func authorization(token string) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// prevents system administrators from accidentally
			// exposing CIHub without credentials.
			if token == "" {
				w.WriteHeader(403)
			} else if token == r.Header.Get("X-CIHub-Token") {
				next.ServeHTTP(w, r)
			} else {
				w.WriteHeader(401)
			}
		})
	}
}
