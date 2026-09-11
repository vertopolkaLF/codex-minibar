use super::*;
use std::collections::HashMap;
use gpui_component::{ActiveTheme, button::{Button,ButtonVariants}, input::{Input,InputState,InputEvent}, switch::Switch};
use serde_json::Value;
use crate::settings::*;
use super::popup_view::card;
const PAGES:&[(&str,&str)]=&[("general","General"),("appearance","Appearance"),("providers","Providers"),("popup","Popup"),("schedule","Limit activation"),("tray","Tray"),("notifications","Notifications"),("advanced","Advanced"),("log","Log"),("about","About")];
pub(super) struct SettingsView {
    model:Entity<Model>, page:&'static str, inputs:HashMap<String,Entity<InputState>>, subscriptions:Vec<Subscription>, error:Option<String>, expanded:Option<String>, confirm:Option<&'static str>, transition:u64,
}
impl SettingsView {
    pub fn new(model:Entity<Model>,window:&mut Window,cx:&mut Context<Self>)->Self {
        let subscription=cx.observe_in(&model,window,|this,_,window,cx| {
            let document=serde_json::to_value(&this.model.read(cx).snapshot.settings).unwrap();
            for (path,input) in &this.inputs {
                if !path.starts_with('/') || input.read(cx).focus_handle(cx).is_focused(window) {continue;}
                if let Some(value)=document.pointer(path) {
                    let text=match value {Value::String(s)=>s.clone(),Value::Null=>String::new(),_=>value.to_string()};
                    if input.read(cx).value().as_ref()!=text {input.update(cx,|input,cx|input.set_value(text,window,cx));}
                }
            }
            cx.notify();
        });
        let onboarding=!model.read(cx).snapshot.settings.onboarding_completed;
        Self{model,page:if onboarding {"providers"}else{"general"},inputs:HashMap::new(),subscriptions:vec![subscription],error:None,expanded:None,confirm:None,transition:0}
    }
    fn patch(&mut self,path:&str,value:Value,cx:&mut Context<Self>) {
        let mut document=serde_json::to_value(&self.model.read(cx).snapshot.settings).expect("serialize settings");
        if let Some(slot)=document.pointer_mut(path) {*slot=value;}else {self.error=Some("This setting is no longer available.".into());cx.notify();return;}
        match serde_json::from_value::<Settings>(document) {
            Ok(settings)=>{self.error=None;self.model.update(cx,|model,cx|model.edit(|s|*s=settings,cx));}
            Err(e)=>{self.error=Some(format!("Invalid value: {e}"));cx.notify();}
        }
    }
    fn field(&mut self,path:String,label:String,value:&Value,window:&mut Window,cx:&mut Context<Self>)->AnyElement {
        let key=path.rsplit('/').next().unwrap_or("");
        if let Value::Bool(value)=value {
            let path2=path.clone();
            return card(cx).child(div().flex().justify_between().items_center().gap_3().child(div().flex_1().child(label))
                .child(switch(path).checked(*value).on_click(cx.listener(move |this,value,_,cx|this.patch(&path2,Value::Bool(*value),cx))))).into_any_element();
        }
        if let Value::Object(fields)=value {
            let mut group=div().flex().flex_col().gap_2().child(div().mt_2().font_weight(FontWeight::SEMIBOLD).child(label));
            for (key,value) in fields {if key=="id" || key=="api_key_ids" {continue;} group=group.child(self.field(format!("{path}/{}",escape(key)),human(key),value,window,cx));}
            return group.into_any_element();
        }
        if let Value::Array(items)=value {
            let mut group=div().flex().flex_col().gap_2().child(div().font_weight(FontWeight::SEMIBOLD).child(label));
            for (index,item) in items.iter().enumerate() {
                let remove_path=path.clone(); let up_path=path.clone();
                group=group.child(div().flex().flex_col().gap_2().p_2().border_1().border_color(cx.theme().border).rounded_md()
                    .child(div().flex().justify_end().gap_2()
                        .child(button(format!("up-{path}-{index}")).label("Move up").on_click(cx.listener(move |this,_,_,cx|{let mut doc=serde_json::to_value(&this.model.read(cx).snapshot.settings).unwrap();if let Some(Value::Array(a))=doc.pointer_mut(&up_path){if index>0 {a.swap(index,index-1);let value=Value::Array(a.clone());this.patch(&up_path,value,cx);}}})))
                        .child(button(format!("remove-{path}-{index}")).label("Remove").on_click(cx.listener(move |this,_,_,cx|{let mut doc=serde_json::to_value(&this.model.read(cx).snapshot.settings).unwrap();if let Some(Value::Array(a))=doc.pointer_mut(&remove_path){if index<a.len(){a.remove(index);let value=Value::Array(a.clone());this.patch(&remove_path,value,cx);}}}))))
                    .child(self.field(format!("{path}/{index}"),format!("{} {}",human(key),index+1),item,window,cx)));
            }
            let template=match key {
                "scheduled_activations"=>Some(serde_json::to_value(ScheduledActivation::default()).unwrap()),
                "auto_activation_pauses"=>Some(serde_json::to_value(AutoActivationPause::default()).unwrap()),
                "tray_widgets"=>Some(serde_json::to_value(TrayWidget::default_user_widget()).unwrap()),
                "indicators"=>Some(serde_json::to_value(TrayIndicator::new(ProviderKind::Codex,"codex.session")).unwrap()),
                "weekdays"=>Some(Value::from(0)),
                _=>None,
            };
            if let Some(template)=template {
                group=group.child(button(format!("add-{path}")).label("Add").on_click(cx.listener(move |this,_,_,cx|{let mut doc=serde_json::to_value(&this.model.read(cx).snapshot.settings).unwrap();if let Some(Value::Array(a))=doc.pointer_mut(&path){a.push(template.clone());let value=Value::Array(a.clone());this.patch(&path,value,cx);}})));
            }
            return group.into_any_element();
        }
        let options=options(key);
        if !options.is_empty() {
            let value_label=value.as_str().map(human).unwrap_or_default();
            let expand_path=path.clone();
            let mut control=card(cx).child(div().flex().items_center().justify_between().gap_2().child(label).child(button(path.clone()).label(value_label).on_click(cx.listener(move |this,_,_,cx|{this.expanded=if this.expanded.as_ref()==Some(&expand_path){None}else{Some(expand_path.clone())};cx.notify();}))));
            if self.expanded.as_ref()==Some(&path) {
                let mut choices=div().flex().flex_wrap().gap_2();
                for option in options {let path=path.clone();choices=choices.child(button(format!("{path}-{option}")).label(human(option)).on_click(cx.listener(move |this,_,_,cx|{this.expanded=None;this.patch(&path,Value::String(option.into()),cx);cx.notify();})));}
                control=control.child(choices);
            }
            return control.into_any_element();
        }
        let input_key=path.clone();
        if !self.inputs.contains_key(&path) {
            let text=match value {Value::String(s)=>s.clone(),Value::Null=>String::new(),_=>value.to_string()};
            let number=value.is_number();let nullable=value.is_null() || key.ends_with("_path");
            let input=cx.new(|cx|InputState::new(window,cx).default_value(text));
            let subscription=cx.subscribe_in(&input,window,move |this,input,event,_,cx| {
                if matches!(event,InputEvent::Blur | InputEvent::PressEnter{..}) {
                    let text=input.read(cx).value().to_string();
                    let value=if number {match text.parse::<i64>() {Ok(v)=>Value::from(v),Err(_)=>{this.error=Some("Enter a whole number.".into());cx.notify();return;}}}
                        else if nullable && text.trim().is_empty() {Value::Null}else{Value::String(text)};
                    this.patch(&input_key,value,cx);
                }
            });
            self.subscriptions.push(subscription);self.inputs.insert(path.clone(),input);
        }
        card(cx).child(div().flex().items_center().justify_between().gap_3().child(div().flex_1().child(label)).child(div().w(px(180.)).child(Input::new(self.inputs.get(&path).unwrap())))).into_any_element()
    }
    fn secret(&mut self,id:String,label:String,provider:ProviderKind,account:Option<(String,Option<String>)>,window:&mut Window,cx:&mut Context<Self>)->AnyElement {
        if !self.inputs.contains_key(&id) {let input=cx.new(|cx|InputState::new(window,cx).masked(true).placeholder("Paste key to replace"));self.inputs.insert(id.clone(),input);}
        let input=self.inputs[&id].clone();let save_input=input.clone(); let clear_account=account.clone();
        card(cx).child(label).child(Input::new(&input))
            .child(div().flex().gap_2()
                .child(button(format!("save-{id}")).primary().label("Save key").on_click(cx.listener(move |this,_,window,cx|{
                    let value=save_input.read(cx).value().to_string();if value.trim().is_empty(){return;}
                    this.save_key(provider,account.clone(),Some(value.trim()),cx);
                    save_input.update(cx,|input,cx|input.set_value("",window,cx));
                })))
                .child(button(format!("clear-{id}")).label("Remove key").on_click(cx.listener(move |this,_,_,cx|this.save_key(provider,clear_account.clone(),None,cx))))).into_any_element()
    }
    fn save_key(&mut self,p:ProviderKind,account:Option<(String,Option<String>)>,value:Option<&str>,cx:&mut Context<Self>) {
        let result=if let Some((account,key))=account {if let Some(key)=key {crate::openrouter::save_account_api_key(&account,&key,value)}else{crate::openrouter::save_management_key(&account,value)}}else{crate::opencode::save_manual_key(p,value)};
        match result {Ok(())=>self.model.update(cx,|model,cx|model.edit(|s|{match p {ProviderKind::OpenCodeZen=>s.opencode_zen_credentials_revision+=1,ProviderKind::OpenCodeGo=>s.opencode_go_credentials_revision+=1,_=>s.openrouter_credentials_revision+=1}},cx)),Err(e)=>self.error=Some(e.to_string())}
        cx.notify();
    }
    fn providers(&mut self,window:&mut Window,cx:&mut Context<Self>)->AnyElement {
        let settings=self.model.read(cx).snapshot.settings.clone();let mut body=div().flex().flex_col().gap_3();
        for provider in ProviderKind::ALL {
            let model=self.model.clone();
            body=body.child(card(cx).child(div().flex().items_center().justify_between().child(provider.display_name())
                .child(switch(format!("provider-{}",provider.id())).checked(settings.providers.is_enabled(provider)).on_click(move |value,_,cx|model.update(cx,|m,cx|m.edit(|s|s.providers.set_enabled(provider,*value),cx))))));
            if !settings.providers.is_enabled(provider){continue;}
            if matches!(provider,ProviderKind::Codex|ProviderKind::Claude|ProviderKind::Cursor) {
                let path=format!("/{}_path",provider.id());let doc=serde_json::to_value(&settings).unwrap();
                body=body.child(self.field(path.clone(),"Application path (automatic when empty)".into(),doc.pointer(&path).unwrap_or(&Value::Null),window,cx));
            }
            if matches!(provider,ProviderKind::OpenCodeZen|ProviderKind::OpenCodeGo) {body=body.child(self.secret(format!("key-{}",provider.id()),"API key".into(),provider,None,window,cx));}
            if provider==ProviderKind::OpenRouter {
                for (index,account) in settings.openrouter_accounts.iter().enumerate() {
                    body=body.child(self.field(format!("/openrouter_accounts/{index}/name"),"Account name".into(),&Value::String(account.name.clone()),window,cx));
                    body=body.child(self.secret(format!("management-{}",account.id),"Management key".into(),provider,Some((account.id.clone(),None)),window,cx));
                    for key in &account.api_key_ids {body=body.child(self.secret(format!("api-{}-{key}",account.id),"API key".into(),provider,Some((account.id.clone(),Some(key.clone()))),window,cx));}
                    let model=self.model.clone();let id=account.id.clone();
                    body=body.child(button(format!("add-key-{}",account.id)).label("Add API key").on_click(move |_,_,cx|model.update(cx,|m,cx|m.edit(|s|{if let Some(a)=s.openrouter_accounts.iter_mut().find(|a|a.id==id){a.api_key_ids.push(OpenRouterAccount::new_api_key_id());}},cx))));
                }
                let model=self.model.clone();body=body.child(button("add-account").label("Add account").on_click(move |_,_,cx|model.update(cx,|m,cx|m.edit(|s|s.openrouter_accounts.push(OpenRouterAccount::default()),cx))));
            }
        }
        body.into_any_element()
    }
}
impl Render for SettingsView {
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        let settings=self.model.read(cx).snapshot.settings.clone();
        let mut sidebar=div().w(px(178.)).flex_none().p_2().flex().flex_col().gap_1().border_r_1().border_color(cx.theme().border);
        for &(id,label) in PAGES {
            sidebar=sidebar.child(div().id(id).p_2().rounded_md().cursor_pointer().bg(if self.page==id{cx.theme().secondary}else{cx.theme().background}).child(label)
                .on_click(cx.listener(move |this,_,_,cx|{this.page=id;this.expanded=None;this.inputs.clear();this.subscriptions.truncate(1);this.transition+=1;cx.notify();})));
        }
        let mut content=div().flex().flex_col().gap_3().p_5().child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(if !settings.onboarding_completed {"Welcome to Codex Minibar"}else{PAGES.iter().find(|p|p.0==self.page).unwrap().1}));
        if let Some(error)=self.error.as_ref().or(self.model.read(cx).snapshot.error.as_ref()) {content=content.child(div().text_color(rgb(0xe16d73)).child(error.clone()));}
        if self.page=="providers" {content=content.child(self.providers(window,cx));}
        else if self.page=="log" {content=content.child(button("open-log").label("Open log").on_click(|_,_,_|{let _=crate::logger::open();})).child(div().text_size(px(11.)).child(crate::logger::tail_lines(150).unwrap_or_default()));}
        else if self.page=="about" {
            let runtime=self.model.read(cx).runtime.clone();
            content=content.child(card(cx).child(format!("Codex Minibar {}",env!("CARGO_PKG_VERSION"))).child("AI usage in your system tray.").child("Built with GPUI"))
                .child(button("check-update").label("Check for updates").on_click(move |_,_,_|runtime.state.updates.check_async(false,false)))
                .child(button("github").label("GitHub").on_click(|_,_,cx|cx.open_url(crate::updater::REPO_URL)));
        } else {
            let fields:&[&str]=match self.page {
                "general"=>&["start_at_login","usage_stats_enabled","limit_refresh_interval","usage_refresh_interval","time_format","history_retention_days","check_for_updates"],
                "appearance"=>&["theme","accent_color","popup_background_material","popup_corner_radius","bottom_bar_size","animations_enabled","use_colored_provider_icons","use_colored_sidebar_icons","replace_chatgpt_logo_with_codex"],
                "popup"=>&["show_used_percentage","show_usage_pace","compact_usage_cards","show_account_name","show_total_spend_on_all_tab","total_spend_presentation","total_spend_period","popup_order","popup_visibility"],
                "schedule"=>&["automatic_activation","scheduled_activations","auto_activation_pauses"],
                "tray"=>&["tray_widgets"],"notifications"=>&["notifications"],_=>&[],
            };
            let doc=serde_json::to_value(&settings).unwrap();
            for &key in fields {if let Some(value)=doc.get(key){content=content.child(self.field(format!("/{key}"),human(key),value,window,cx));}}
            if self.page=="advanced" {
                content=content.child(button("export-settings").label("Copy settings").on_click({let model=self.model.clone();move |_,_,cx| {if let Ok(text)=toml::to_string_pretty(&model.read(cx).snapshot.settings){cx.write_to_clipboard(ClipboardItem::new_string(text));}}}))
                    .child(button("import-settings").label("Import settings from clipboard").on_click(cx.listener(|this,_,_,cx|{if let Some(text)=cx.read_from_clipboard().and_then(|c|c.text()){match toml::from_str::<Settings>(&text){Ok(next)=>this.model.update(cx,|m,cx|m.edit(|s|*s=next,cx)),Err(e)=>{this.error=Some(e.to_string());cx.notify();}}}})))
                    .child(button("clear-usage").label("Clear usage data").on_click(cx.listener(|this,_,_,cx|{this.confirm=Some("clear");cx.notify();})))
                    .child(button("reset-settings").label("Reset settings").on_click(cx.listener(|this,_,_,cx|{this.confirm=Some("reset");cx.notify();})));
                if let Some(action)=self.confirm {content=content.child(card(cx).child("This action cannot be undone.")
                    .child(button("confirm-action").label("Confirm").on_click(cx.listener(move |this,_,_,cx|{if action=="clear"{this.model.read(cx).runtime.send(Command::ClearUsage);}else{this.model.update(cx,|m,cx|m.edit(|s|*s=Settings{onboarding_completed:true,..Default::default()},cx));}this.confirm=None;cx.notify();})))
                    .child(button("cancel-action").label("Cancel").on_click(cx.listener(|this,_,_,cx|{this.confirm=None;cx.notify();}))));}
            }
        }
        if !settings.onboarding_completed {let model=self.model.clone();content=content.child(button("done").primary().label("Done").on_click(move |_,window,cx|{model.update(cx,|m,cx|m.edit(|s|s.onboarding_completed=true,cx));window.remove_window();}));}
        div().size_full().flex().bg(cx.theme().background).text_color(cx.theme().foreground).text_size(px(13.)).child(sidebar)
            .child(div().id(SharedString::from(format!("settings-scroll-{}",self.page))).flex_1().min_w_0().overflow_y_scroll().child(content))
    }
}
fn escape(value:&str)->String {value.replace('~',"~0").replace('/',"~1")}
fn human(key:&str)->String {
    match key {"hour_12"=>"12-hour clock".into(),"hour_24"=>"24-hour clock".into(),"minute1"=>"1 minute".into(),"seconds30"=>"30 seconds".into(),"provider_id"=>"Provider".into(),"metric_id"=>"Metric".into(),"all_tab"=>"Home tab".into(),"provider_tab"=>"Provider tab".into(),"time_minutes"=>"Time (minutes after midnight)".into(),_=>{let s=key.replace('_'," ");let mut c=s.chars();c.next().map(|first|first.to_uppercase().collect::<String>()+c.as_str()).unwrap_or_default()}}
}
fn options(key:&str)->Vec<&'static str> {
    match key {
        "theme"=>vec!["auto","light","dark"],"accent_color"=>vec!["windows","blue","purple","pink","red","orange","green","teal"],
        "popup_background_material"=>vec!["acrylic","mica"],"popup_corner_radius"=>vec!["zero","four","small","medium","large","extra_large"],
        "bottom_bar_size"=>vec!["comfortable","compact"],"time_format"=>vec!["hour_12","hour_24"],
        "limit_refresh_interval"=>vec!["seconds30","minute1","minutes5","minutes10","minutes15"],
        "usage_refresh_interval"=>vec!["minute1","minutes5","minutes10","minutes15","minutes30","minutes45","minutes60"],
        "total_spend_presentation"=>vec!["donut","progress_bar"],"total_spend_period"=>vec!["today","yesterday","thirty_days"],
        "provider_id"=>ProviderKind::ALL.iter().map(|p|p.id()).collect(),
        "kind"=>vec!["limits","app_icon"],"limit_value"=>vec!["remaining","used"],"color_mode"=>vec!["status","fixed","provider","accent","monochrome"],
        "presentation"=>vec!["stacked_numbers","stacked_bars","nested_rings","reset_time","reset_countdown"],_=>vec![]
    }
}

fn button(id:impl Into<SharedString>)->Button {Button::new(id.into())}
fn switch(id:impl Into<SharedString>)->Switch {Switch::new(id.into())}

