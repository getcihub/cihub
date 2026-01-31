package runners

import (
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

// HandleList returns an http.HandlerFunc that writes a json-encoded
// list of runner history to the response body.
func HandleList(runners core.RunnerStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		results, err := runners.ListStatus(r.Context(), core.RunnerStatusBusy)
		if err != nil {
			render.InternalErrorf(w, err.Error())
			logger.FromRequest(r).
				WithError(err).
				Debugln("api: cannot list machine runners")
			return
		}

		render.OK(w, render.ReasonListed, results)
	}
}
