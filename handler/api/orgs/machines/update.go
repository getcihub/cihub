package machines

import (
	"net/http"

	"github.com/getcihub/cihub/handler/api/render"
)

// HandleUpdate returns an http.HandlerFunc that processes http
// requests to update a machine.
func HandleUpdate() http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		render.NotImplemented(w)
	}
}
