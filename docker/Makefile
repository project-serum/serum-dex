IMG_ORG ?= projectserum
IMG_VER ?= latest

WORKDIR=$(PWD)

.PHONY: development development-push development-shell

default:

development: development/Dockerfile
	@docker build $@ -t $(IMG_ORG)/$@:$(IMG_VER)

development-push:
	@docker push $(IMG_ORG)/development:$(IMG_VER)

development-shell:
	@docker run -ti --rm --net=host \
		-v $(WORKDIR)/..:/workdir \
		$(IMG_ORG)/development:$(IMG_VER) bash
