package acl

import (
	"net/http"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
	"github.com/getcihub/cihub/logger"
)

// staleMembershipDuration is the maximum duration to sync membership.
const staleMembershipDuration = time.Minute * 10

// InjectOrganization returns an http.Handler middleware that injects
// the organization and memberships into the context.
func InjectOrganization(
	installations core.InstallationStore,
	installationz core.InstallationService,
	memberships core.MembershipStore,
) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			namespace := chi.URLParam(r, "namespace")
			log := logger.FromRequest(r)
			ctx := r.Context()

			user, ok := request.UserFrom(ctx)
			if !ok {
				render.Unauthorized(w)
				log.Debugln("api: authentication required for access")
				return
			}

			log = log.
				WithField("user.admin", user.Admin).
				WithField("namespace", namespace)

			installation, err := installations.FindLogin(ctx, namespace)
			if err != nil {
				render.NotFound(w)
				log.WithError(err).
					Debugln("api: installation not found")
				return
			}

			// the installation is stored in the request context
			// and can be accessed by subsequent handlers in the
			// request chain.
			ctx = request.WithInstallation(ctx, installation)

			// if the user is an administrator they are always
			// granted access to the organization data.
			if user.Admin {
				next.ServeHTTP(w, r.WithContext(ctx))
				return
			}

			// else get the cached membership from the database
			// for the user and installation.
			membership, err := memberships.Find(ctx, installation.ID, user.ID)
			if err != nil {
				// if the membership is not found we forward
				// the request to the next handler in the chain
				// with no membership in the context.
				//
				// It is the responsibility to downstream
				// middleware and handlers to decide if the
				// request should be rejected.
				next.ServeHTTP(w, r.WithContext(ctx))
				return
			}

			log = log.WithFields(
				logrus.Fields{
					"role":  membership.Role,
					"state": membership.State,
				},
			)

			// update logger for next handlers
			ctx = logger.WithContext(ctx, log)

			// no need to sync membership for account owners
			if membership.Role == core.MembershipRoleOwner {
				ctx = request.WithMembership(ctx, membership)
				next.ServeHTTP(w, r.WithContext(ctx))
				return
			}

			// because the membership is synced with GitHub they may be stale.
			// If the membership is stale they are refreshed below.
			if membership.Synced == 0 || time.Unix(membership.Synced, 0).Add(staleMembershipDuration).Before(time.Now()) {
				log.Debugln("api: update membership permission")

				membershipv, err := installationz.FindMembership(ctx, user, installation.Login)
				if err != nil {
					render.NotFound(w)
					log.WithError(err).
						Warnln("api: cannot sync org membership")
					return
				}

				membership.Synced = time.Now().Unix()
				membership.State = membershipv.State
				membership.Role = membershipv.Role

				err = memberships.Update(ctx, membership)
				if err != nil {
					log.WithError(err).Debugln("api: cannot cache installation membership")
				} else {
					log.Debugln("api: installation membership synchronized")
				}
			}

			ctx = request.WithMembership(ctx, membership)
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}
