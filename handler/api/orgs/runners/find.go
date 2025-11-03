package runners

import (
	"net/http"

	"github.com/go-chi/chi/v5"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

// HandleFind returns an http.HandlerFunc that writes the
// json-encoded runner details to the response body.
func HandleFind(runners core.RunnerStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		var name = chi.URLParam(r, "name")

		runner, err := runners.Find(r.Context(), name)
		if err != nil {
			render.NotFound(w)
			logger.FromRequest(r).
				WithError(err).
				WithField("name", name).
				Debugln("api: runner not found")
			return
		}

		render.OK(w, render.ReasonResolved, runner)
	}
}
