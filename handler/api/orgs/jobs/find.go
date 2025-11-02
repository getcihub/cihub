package jobs

import (
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
)

// HandleFind returns an http.HandlerFunc that writes the
// json-encoded job details to the response body.
func HandleFind(jobs core.JobStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		render.NotImplemented(w)
	}
}
