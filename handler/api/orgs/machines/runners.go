package machines

import (
	"net/http"

	"github.com/getcihub/cihub/handler/api/render"
)

// HandleRunners returns an http.HandlerFunc that processes http
// requests to list all runners of a machine.
func HandleRunners() http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		render.NotImplemented(w)
	}
}
