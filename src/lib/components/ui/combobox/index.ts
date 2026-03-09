import Root from "./combobox.svelte";
import Input from "./combobox-input.svelte";
import Trigger from "./combobox-trigger.svelte";
import Content from "./combobox-content.svelte";
import Item from "./combobox-item.svelte";

// Re-use Select's group components since Combobox shares them
import Group from "../select/select-group.svelte";
import GroupHeading from "../select/select-group-heading.svelte";
import Separator from "../select/select-separator.svelte";

export {
	Root,
	Input,
	Trigger,
	Content,
	Item,
	Group,
	GroupHeading,
	Separator,
	//
	Root as Combobox,
	Input as ComboboxInput,
	Trigger as ComboboxTrigger,
	Content as ComboboxContent,
	Item as ComboboxItem,
	Group as ComboboxGroup,
	GroupHeading as ComboboxGroupHeading,
	Separator as ComboboxSeparator,
};
