package orgs

import (
	"net/http"

	"github.com/getcihub/cihub/handler/api/render"
)

// HandleList returns an http.HandlerFunc that processes http
// requests to list all organizations in the database.
func HandleList() http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		render.NotImplemented(w)
	}
}
