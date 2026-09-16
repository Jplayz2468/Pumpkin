#[allow(clippy::wildcard_imports)]
use super::*;
use pumpkin_inventory::Inventory;

impl JavaClient {
    pub fn handle_edit_book(&self, player: &Player, packet: &SEditBook<'_>) {
        let slot = packet.slot.0;
        if !(0..9).contains(&slot) && slot != 40 {
            return;
        }
        let held_stack = player.inventory().get_stack(slot as usize);
        if held_stack.is_empty()
            || held_stack
                .get_data_component::<WritableBookContentImpl>()
                .is_none()
        {
            return;
        }

        let mut pages: Vec<String> = packet.pages.iter().map(|p| (*p).to_string()).collect();
        let mut title = packet.title.map(std::string::ToString::to_string);
        let signing = title.is_some();

        if let Some(player_arc) = player.world().get_player_by_uuid(player.gameprofile.id)
            && let Some(server) = player.world().server.upgrade()
        {
            let mut event =
                crate::plugin::api::events::player::player_edit_book::PlayerEditBookEvent {
                    player: player_arc,
                    slot: slot as u32,
                    pages: pages.clone(),
                    title: title.clone(),
                    signing,
                    cancelled: false,
                };
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
            pages = event.pages;
            title = event.title;
        }

        if let Some(title) = title {
            let mut written_book = ItemStack::new(held_stack.item_count, &Item::WRITTEN_BOOK);
            for (kind, value) in held_stack.patch {
                if let Some(value) = value {
                    written_book.set_data_component_dyn(value);
                } else {
                    written_book.remove_data_component(kind);
                }
            }
            written_book.remove_data_component(DataComponent::WritableBookContent);
            let content = WrittenBookContentImpl {
                title: title.into(),
                author: player.gameprofile.name.clone(),
                pages: pages
                    .into_iter()
                    .map(|p| TextComponent::text(p).into())
                    .collect(),
                generation: 0,
                resolved: true,
            };
            written_book.set_data_component(content);
            player.inventory().set_stack(slot as usize, written_book);
        } else {
            let mut writable_book = held_stack;
            let content = WritableBookContentImpl {
                pages: pages.into_iter().map(Into::into).collect(),
            };
            writable_book.set_data_component(content);
            player.inventory().set_stack(slot as usize, writable_book);
        }
    }
}
