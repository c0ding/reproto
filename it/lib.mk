M := $(notdir $(CURDIR))
M := $(M:test-%=%)

ifeq ($(REPROTO),)
$(error "REPROTO: missing variable")
endif

ifeq ($(PROJECTS),)
$(error "PROJECTS: missing variable")
endif

ifeq ($(M),)
$(error "M: missing variable")
endif

reproto-args := $(REPROTO_ARGS)

ifeq ($(DEBUG),yes)
reproto-args := $(reproto-args) --debug
endif

target-file ?= Makefile

RM := rm -rf
CP := cp -ra
DIFF ?= diff
RSYNC ?= rsync
REPROTO ?= $(default-reproto)
CARGO ?= cargo
# projects that are excluded
EXCLUDE ?=

expected := expected
output := output
workdir := workdir
input := input
targets := test

python-args :=
java-args := -m builder
js-args :=
rust-args :=
doc-args :=
suites := python java js rust doc

paths := proto
exclude-projects :=
exclude-suites :=

include $(target-file)

suites := $(filter-out $(EXCLUDE) $(exclude-suites), $(suites))
projects := $(filter-out $(EXCLUDE) $(exclude-projects), $(PROJECTS))

for-each-suite = $(foreach suite,$(suites),$(eval $(call $1,$(suite))))
for-each-project = $(foreach project,$(projects),$(eval $(call $1,$(project))))

compile-args := $(paths:%=--path %) $(targets:%=--package %)

# how to build suites
java-suite := java $(compile-args) $(java-args)
js-suite := js $(compile-args) $(js-args)
python-suite := python $(compile-args) $(python-args)
rust-suite := rust $(compile-args) $(rust-args)
doc-suite := doc $(compile-args) --skip-static $(doc-args)

# how to build projects
java-project := java -m fasterxml $(compile-args) -o $(workdir)/java/target/generated-sources/reproto
js-project := js $(compile-args) -o $(workdir)/js/generated
python-project := python $(compile-args) -o $(workdir)/python/generated
rust-project := rust $(compile-args) -o $(workdir)/rust/src --package-prefix generated

# base command invocations
reproto-cmd := $(REPROTO) $(reproto-args)
reproto-compile := $(reproto-cmd) compile

input-files := $(shell ls -1 $(input))
diff-dirs = $(DIFF) -ur $(1) $(2)

define sync-dirs
	$(RM) $(2)
	$(CP) $(1) $(2)
endef

define suite-targets
suite-build += suite-build/$1
suite-update += suite-update/$1
suite-diff += suite-diff/$1

suite-build/$1: $$(REPROTO) $$(output)/suite/$1
	@echo "$$(M): Suite: $1"
	$$(RM) $$(output)/suite/$1
	$$(reproto-compile) $$($1-suite) -o $$(output)/suite/$1

$$(expected)/suite/$1:
	mkdir -p $$@

suite-update/$1: suite-build/$1 $$(expected)/suite/$1
	@echo "$$(M): Updating Suite: $1"
	$(call sync-dirs,$$(output)/suite/$1,$$(expected)/suite/$1)

suite-diff/$1: suite-build/$1 $$(expected)/suite/$1
	@echo "$$(M): Verifying Diff: $1"
	$(call diff-dirs,$$(expected)/suite/$1,$$(output)/suite/$1)
endef

define project-run-target
project-run += project-run/$1/$2
project-run-$1 += project-run/$1/$2

$(output)/project/$1:
	mkdir -p $@

project-run/$1/$2: $(workdir)/$1/script.sh $(output)/project/$1
	@echo "$(M): Running Project: $1 (against $(input)/$2)"
	$(workdir)/$1/script.sh < $(input)/$2 > $(output)/project/$1/$2
endef

define project-targets
project-workdir += $$(workdir)/$1
project-script += $$(workdir)/$1/script.sh
project-update += project-update/$1
project-diff += project-diff/$1

$$(workdir)/$1: $$(workdir)
	$(call sync-dirs,../$1,$$@)

$$(workdir)/$1/script.sh: $$(workdir)/$1
	@echo "$$(M): Building Project: $1"
	$$(reproto-compile) $$($1-project)
	$$(MAKE) -C $$(workdir)/$1

$(foreach i,$(input-files),$(call project-run-target,$1,$i))

$$(expected)/project/$1:
	mkdir -p $$@

project-update/$1: $$(project-run-$1) $$(expected)/project/$1
	@echo "$$(M): Updating Project: $1"
	$(call sync-dirs,$$(output)/project/$1,$$(expected)/project/$1)

project-diff/$1: $$(project-run-$1) $$(expected)/project/$1
	@echo "$$(M): Diffing Project: $1"
	$(call diff-dirs,$$(expected)/project/$1,$$(output)/project/$1)
endef

.DEFAULT: all

$(call for-each-suite,suite-targets)
$(call for-each-project,project-targets)

all: suites projects

clean: clean-projects clean-suites

update: update-suites update-projects

suites: $(suite-diff)

clean-suites:
	$(RM) $(output)/suite

update-suites: $(suite-update)

projects: $(project-diff)

clean-projects:
	$(RM) $(workdir)
	$(RM) $(output)/project-*

update-projects: $(project-update)

$(workdir) $(input):
	mkdir -p $@

# treating script as phony will cause them to rebuilt
ifeq ($(REBUILD),yes)
.PHONY: $(project-script)
endif

.PHONY: all clean update
.PHONY: projects clean-projects update-projects
.PHONY: suites clean-suites update-suites
.PHONY: $(suite-build) $(suite-update) $(suite-diff) $(project-run) $(project-update) $(project-diff)
