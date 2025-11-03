package jobs

import (
	"net/http"
	"strconv"

	"github.com/go-chi/chi/v5"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

// HandleFind returns an http.HandlerFunc that writes the
// json-encoded job details to the response body.
func HandleFind(jobs core.JobStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		var (
			id    = chi.URLParam(r, "id")
			owner = chi.URLParam(r, "owner")
		)

		jobID, err := strconv.ParseInt(id, 10, 64)
		if err != nil {
			render.BadRequest(w)
			logger.FromRequest(r).
				WithError(err).
				WithField("id", id).
				Debugln("api: cannot parse job ID")
			return
		}

		job, err := jobs.Find(r.Context(), owner, jobID)
		if err != nil {
			render.NotFound(w)
			logger.FromRequest(r).
				WithError(err).
				WithField("owner", owner).
				WithField("id", jobID).
				Debugln("api: job not found")
			return
		}

		render.OK(w, render.ReasonResolved, job)
	}
}
