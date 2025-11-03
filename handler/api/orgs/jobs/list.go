package jobs

import (
	"net/http"
	"strconv"

	"github.com/go-chi/chi/v5"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

// HandleList returns an http.HandlerFunc that writes a json-encoded
// paginated list of jobs from an owner to the response body.
func HandleList(jobs core.JobStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		var (
			owner  = chi.URLParam(r, "owner")
			status = r.FormValue("status")
			limit  = r.FormValue("limit")
			jobID  = r.FormValue("job_id")
		)

		// Convert jobID string, used if defined
		offset, _ := strconv.Atoi(jobID)

		// Convert limit
		perPage, _ := strconv.Atoi(limit)
		if perPage < 1 || perPage > 100 {
			perPage = 25
		}

		// Check for status
		switch status {
		case "completed", "incomplete":
		default:
			status = "incomplete"
		}

		var (
			err     error
			results []*core.Job
		)

		switch {
		case status == "completed" && jobID != "":
			results, err = jobs.ListCompleted(r.Context(), owner, perPage, offset)
		case status == "completed" && jobID == "":
			results, err = jobs.ListCompleted(r.Context(), owner, perPage, 0)
		default:
			results, err = jobs.ListIncomplete(r.Context(), owner)
		}

		if err != nil {
			render.InternalErrorf(w, err.Error())
			logger.FromRequest(r).
				WithError(err).
				WithField("owner", owner).
				WithField("status", status).
				Debugln("api: cannot list jobs")
			return
		}

		hasMore := len(results) > perPage
		if hasMore {
			results = results[:perPage]
		}

		render.Paginated(w, results, hasMore)
	}
}
