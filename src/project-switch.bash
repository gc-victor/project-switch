project-switch() {
    if [[ "$1" == "init" ]]; then
        __path__/project-switch init
        return
    fi

    if [[ "$1" == "add" ]]; then
        __path__/project-switch add "$2"
        return
    fi

    if [[ "$1" == "list" && "$2" == "--raw" ]]; then
        __path__/project-switch list --raw
        return
    fi

    if [[ "$1" == "list" ]]; then
        __path__/project-switch list
        return
    fi

    if [[ "$1" == "remove" ]]; then
        __path__/project-switch remove "$2"
        return
    fi

    if [[ "$1" == "dir" ]]; then
        __path__/project-switch dir "$2"
        return
    fi

    local project_name="$2"
    local project_path=$(__path__/project-switch dir "$project_name")

    if [[ "$project_path" != "" ]]; then
        cd "$project_path"
        echo "Switched to project: $project_name"
    else
        echo "Project $project_name not found"
    fi
}
