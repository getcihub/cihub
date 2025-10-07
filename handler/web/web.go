package web

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/unrolled/secure"
)

type Server struct {
	Options secure.Options
}

// Server is a http.Handler which exposes CIHub UI over HTTP.
func New(options secure.Options) Server {
	return Server{
		Options: options,
	}
}

// Handler returns an http.Handler
func (s Server) Handler() http.Handler {
	r := chi.NewRouter()

	r.Use(middleware.Recoverer)
	r.Use(middleware.NoCache)
	r.Use(middleware.StripSlashes)

	security := secure.New(s.Options)
	r.Use(security.Handler)

	return r
}
