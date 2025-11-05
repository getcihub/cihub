package machines

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

// HandleDelete returns an http.HandlerFunc that handles an
// http.Request to delete a machine entry from the datastore.
func HandleDelete(
	machines core.MachineStore,
	runners core.RunnerStore,
	scheduler core.Scheduler,
) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		var (
			ctx   = r.Context()
			log   = logger.FromContext(ctx)
			owner = chi.URLParam(r, "owner")
			name  = chi.URLParam(r, "name")
		)

		log = log.WithFields(
			logrus.Fields{
				"owner": owner,
				"name":  name,
			},
		)

		machine, err := machines.Find(ctx, owner, name)
		if err != nil {
			render.NotFound(w)
			log.WithError(err).
				Debugln("api: machine not found")
			return
		}

		runnersToCancel, err := runners.ListMachine(ctx, machine)
		if err != nil {
			render.InternalErrorf(w, err.Error())
			log.WithError(err).
				Warnln("api: cannot get runners to cancel")
			return
		}

		// Foreach runner on the machine, cancel them
		for _, runner := range runnersToCancel {
			log.WithField("runner", runner.Name).Infoln("api: cancelling runner")
			err := scheduler.Cancel(ctx, runner.Name)
			if err != nil {
				render.InternalErrorf(w, err.Error())
				log.WithError(err).
					WithField("runner", runner.Name).
					Warnln("api: machine not found")
				return
			}
		}

		// Delete machine from datastore
		err = machines.Delete(ctx, machine)
		if err != nil {
			render.InternalErrorf(w, err.Error())
			log.WithError(err).
				Warnln("api: cannot delete machine")
			return
		}

		render.OK(w, render.ReasonDeleted, nil)
	}
}
