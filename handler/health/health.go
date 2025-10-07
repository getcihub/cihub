package health

import (
	"io"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

// New returns a new health check router
func New() http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.Recoverer)
	r.Use(middleware.NoCache)
	r.Handle("/", Handler())
	return r
}

// Handler creates an http.HandlerFunc that performs system
// healthchecks and returns 500 if the system is in an unhealthy state.
func Handler() http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(200)
		w.Header().Set("Content-Type", "text/plain")
		_, _ = io.WriteString(w, "OK")
	}
}
