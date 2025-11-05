package rpc

import (
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

// HandleLeave returns an http.HandlerFunc that makes an
// http.Request to leave the cluster.
//
// POST /rpc/v2/leave
func HandleLeave(machines core.MachineStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()
		machine, _ := MachineFrom(ctx)
		log := logger.FromContext(ctx)

		machine.LastSeen = time.Now().Unix()
		machine.Status = core.MachineStatusOffline
		machine.Updated = time.Now().Unix()

		err := machines.Update(ctx, machine)
		if err != nil {
			writeError(w, err)
			log.WithError(err).
				Warnln("rpc: machine cannot leave cluster")
			return
		}

		log.Debugln("rpc: machine leaved cluster")

		writeOK(w)
	}
}
