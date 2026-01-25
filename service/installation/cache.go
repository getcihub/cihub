package installation

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/getcihub/cihub/core"
	lru "github.com/hashicorp/golang-lru"
)

// content key pattern used in the cache, comprised of the
// installation login and user login.
const contentKey = "%s/%s"

type cacher struct {
	mu sync.Mutex

	base core.InstallationService
	size int
	ttl  time.Duration

	cache *lru.Cache
}

type item struct {
	expiry        time.Time
	admin, member bool
}

// NewCache wraps the service with a simple cache to store installation membership.
func NewCache(base core.InstallationService, size int, ttl time.Duration) core.InstallationService {
	cache, _ := lru.New(size)

	return &cacher{
		base:  base,
		cache: cache,
		size:  size,
		ttl:   ttl,
	}
}

func (c *cacher) List(ctx context.Context, user *core.User) ([]*core.Installation, error) {
	return c.base.List(ctx, user)
}

func (c *cacher) Membership(ctx context.Context, user *core.User, name string) (bool, bool, error) {
	key := fmt.Sprintf(contentKey, user.Login, name)
	now := time.Now()

	// get the membership details from cache.
	cached, ok := c.cache.Get(key)
	if ok {
		item := cached.(*item)
		if now.After(item.expiry) {
			c.cache.Remove(cached)
		} else {
			return item.member, item.admin, nil
		}
	}

	// get up-to-date membership details due to cache
	// miss or expired cache item.
	member, admin, err := c.base.Membership(ctx, user, name)
	if err != nil {
		return false, false, err
	}

	c.cache.Add(key, &item{
		expiry: now.Add(c.ttl),
		member: member,
		admin:  admin,
	})

	return member, admin, nil
}
