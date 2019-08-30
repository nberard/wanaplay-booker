import hashlib
import json
from collections import defaultdict

from defaultlist import defaultlist
from telegram import (
    InlineKeyboardMarkup,
    InlineKeyboardButton,
)

import config
from telegram.ext import (
    Updater,
    CommandHandler,
    MessageHandler,
    Filters,
    CallbackQueryHandler,
)
import logging
import requests
from datetime import datetime, timedelta, timezone, time, date

logging.basicConfig(
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s", level=logging.INFO
)
logger = logging.getLogger(__name__)
updater = Updater(token=config.token, use_context=True)
dispatcher = updater.dispatcher


def get_bookings():
    response = requests.get("{}/bookings".format(config.booker_api))
    bookings = json.loads(response.content)
    print(bookings)
    return bookings


def get_bookings_md(bookings):
    if len(bookings) == 0:
        text = "no bookings found"
    else:
        text = ""
        for booking in bookings:
            text += "{} at {} |   {}  \n".format(
                datetime.strptime(booking["date"], "%d/%m/%Y").strftime("%a %d/%m"),
                booking["court_time"],
                booking["court_number"],
            )
    return text


def bookings(update, context):
    context.bot.send_message(
        chat_id=update.message.chat_id,
        text="""
<pre>
     Booking       | Court #
 ----------------- | --------
"""
        + get_bookings_md(get_bookings())
        + """
</pre>
""",
        parse_mode="html",
    )


bookings_handler = CommandHandler("bookings", bookings)
dispatcher.add_handler(bookings_handler)


def get_bots_md(bots):
    if len(bots) == 0:
        text = "no bots found"
    else:
        text = ""
        for bot in bots:
            text += "{} | {}\n".format(
                bot["name"].ljust(19, " "),
                " ☑ " if bot["status"] == "Running" else " ☐ ",
            )
    return text


def get_bots():
    response = requests.get("{}/bots".format(config.booker_api))
    return json.loads(response.content)


def bots(update, context):
    bots = get_bots()
    context.bot.send_message(
        chat_id=update.message.chat_id,
        text="""
<pre>
       Name         | Status
 ------------------ | ------
"""
        + get_bots_md(bots)
        + """
</pre>
    """,
        parse_mode="html",
    )


bots_handler = CommandHandler("bots", bots)
dispatcher.add_handler(bots_handler)


def handle_response(bot, response, chat_id):
    print("response code = {}".format(response.status_code))
    status = (
        "ok" if response.status_code >= 200 and response.status_code < 300 else "ko"
    )
    bot.send_message(chat_id=chat_id, text=status)


def deploy(update, context):
    response = requests.post("{}/bots/actions/deploy".format(config.booker_api))
    handle_response(context.bot, response, update.message.chat_id)


deploy_handler = CommandHandler("deploy", deploy)
dispatcher.add_handler(deploy_handler)


def add(update, context):
    ik_formatter = InlineKeyboardFormatter(3)
    for day in [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ]:
        ik_formatter.add_ik_button(day, {"action": "add", "day": day.lower()})

    context.bot.send_message(
        chat_id=update.message.chat_id,
        text="choose a day",
        reply_markup=InlineKeyboardMarkup(inline_keyboard=ik_formatter.inline_keyboard),
    )


def choose_slot_callback(bot, chat_id, day):
    ik_formatter = InlineKeyboardFormatter(6)
    slot_time = datetime(1, 1, 1, 9, 0, 0)
    while slot_time <= datetime(1, 1, 1, 23, 0, 0):
        ik_formatter.add_ik_button(
            slot_time.strftime("%H:%M"),
            {
                "action": "add",
                "bot_name": "bot_{}_{}".format(day, slot_time.strftime("%H_%M")),
            },
        )
        slot_time = slot_time + timedelta(minutes=40)

    bot.send_message(
        chat_id=chat_id,
        text="choose a time slot",
        reply_markup=InlineKeyboardMarkup(inline_keyboard=ik_formatter.inline_keyboard),
    )


def add_bot_callback(bot, chat_id, bot_name):
    print(bot_name)
    bot_parts = bot_name.split("_")
    payload = {
        "name": bot_name,
        "week_day": bot_parts[1].capitalize(),
        "court_time": "{}:{}".format(bot_parts[2], bot_parts[3]),
        "status": "Created",
    }
    print(payload)
    response = requests.post("{}/bots".format(config.booker_api), json=payload)
    handle_response(bot, response, chat_id)


add_handler = CommandHandler("add", add)
dispatcher.add_handler(add_handler)


def delete(update, context):
    ik_formatter = InlineKeyboardFormatter(2)
    bots = get_bots()
    for bot in bots:
        ik_formatter.add_ik_button(
            bot["name"], {"action": "delete", "bot_name": bot["name"]}
        )
    context.bot.send_message(
        chat_id=update.message.chat_id,
        text="choose a bot to delete",
        reply_markup=InlineKeyboardMarkup(inline_keyboard=ik_formatter.inline_keyboard),
    )


def delete_callback(bot, chat_id, bot_name):
    print(bot_name)
    response = requests.delete("{}/bots/{}".format(config.booker_api, bot_name))
    handle_response(bot, response, chat_id)


delete_handler = CommandHandler("delete", delete)
dispatcher.add_handler(delete_handler)


class InlineKeyboardFormatter:
    def __init__(self, items_max_per_row):
        self.inline_keyboard = defaultlist(list)
        self.current_row = 0
        self.items_max_per_row = items_max_per_row
        self.items_on_current_row = 0

    def go_next_line(self):
        self.current_row += 1
        self.items_on_current_row = 0

    def add_ik_button(self, text, data):
        # print(
        #     f"start current_row={self.current_row} / items_on_current_row={self.items_on_current_row}"
        # )
        if self.items_on_current_row >= self.items_max_per_row:
            self.current_row += 1
            self.items_on_current_row = 0
        self.inline_keyboard[self.current_row].append(
            InlineKeyboardButton(text, callback_data=json.dumps(data))
        )
        self.items_on_current_row += 1


def cancel_dialog(update, context):
    inline_keyboard = list_bookings_selection("cancel")

    context.bot.send_message(
        chat_id=update.message.chat_id,
        text="choose a booking to cancel",
        reply_markup=InlineKeyboardMarkup(inline_keyboard=inline_keyboard),
    )


def cancel_callback(bot, chat_id, booking_id):
    response = requests.delete("{}/bookings/{}".format(config.booker_api, booking_id))
    handle_response(bot, response, chat_id)


cancel_handler = CommandHandler("cancel", cancel_dialog)
dispatcher.add_handler(cancel_handler)


def list_bookings_selection(action):
    ik_formatter = InlineKeyboardFormatter(2)
    bookings = get_bookings()
    bookings_by_day = defaultdict(list)
    for idx, booking in enumerate(bookings):
        bookings_by_day[booking["date"]].append(booking)
    print(len(bookings_by_day))
    for date, day_bookings in bookings_by_day.items():
        booking_date = datetime.strptime(date, "%d/%m/%Y").strftime("%a %d")
        start = min(day_bookings, key=lambda dict: dict["court_time"])["court_time"]
        end = (
            datetime.strptime(
                max(day_bookings, key=lambda dict: dict["court_time"])["court_time"],
                "%H:%M",
            )
            + timedelta(minutes=40)
        ).strftime("%H:%M")
        ik_formatter.add_ik_button(
            "{} {}->{}".format(booking_date, start, end),
            {"action": action, "bookings": [booking["id"] for booking in day_bookings]},
        )
    return ik_formatter.inline_keyboard


def accept_dialog(update, context):
    inline_keyboard = list_bookings_selection("accept")

    context.bot.send_message(
        chat_id=update.message.chat_id,
        text="choose a court period to get invite",
        reply_markup=InlineKeyboardMarkup(inline_keyboard=inline_keyboard),
    )


def accept_callback(bot, chat_id, ids):
    bookings = [booking for booking in get_bookings() if booking["id"] in ids]
    logger.info(bookings)
    start = min(bookings, key=lambda dict: dict["court_time"])
    end = max(bookings, key=lambda dict: dict["court_time"])
    start = datetime.strptime(
        "{} {}".format(start["date"], start["court_time"]), "%d/%m/%Y %H:%M"
    )
    end = datetime.strptime(
        "{} {}".format(end["date"], end["court_time"]), "%d/%m/%Y %H:%M"
    ) + timedelta(minutes=40)
    with open("invite.squash.ics.template", "r") as template_handle, open(
        "invite.squash.ics", "w"
    ) as to_send_handle:
        start_str = start.replace(tzinfo=timezone.utc).strftime("%Y%m%dT%H%M%S")
        end_str = end.replace(tzinfo=timezone.utc).strftime("%Y%m%dT%H%M%S")
        id = hashlib.md5("{}-{}".format(start_str, end_str).encode("utf-8")).hexdigest()
        data = (
            template_handle.read()
            .replace("{{start}}", start_str)
            .replace("{{end}}", end_str)
            .replace("{{id}}", id)
        )
        to_send_handle.write(data)
    response = requests.post(
        "https://api.telegram.org/bot{}/sendDocument".format(config.token),
        files={"document": open("invite.squash.ics", "rb")},
        data={"chat_id": chat_id},
    )
    handle_response(bot, response, chat_id)


accept_handler = CommandHandler("accept", accept_dialog)
dispatcher.add_handler(accept_handler)


def echo(update, context):
    logger.info('echo "{}"'.format(update.message.text))
    context.bot.send_message(chat_id=update.message.chat_id, text=update.message.text)


echo_handler = MessageHandler(Filters.text, echo)
dispatcher.add_handler(echo_handler)


def callback_manager(update, callback_context):
    data = json.loads(update.callback_query["data"])
    if data["action"] == "accept":
        accept_callback(
            callback_context.bot,
            update.callback_query.message.chat.id,
            data["bookings"],
        )
    elif data["action"] == "cancel":
        cancel_callback(
            callback_context.bot,
            update.callback_query.message.chat.id,
            data["bookings"][0],
        )
    elif data["action"] == "delete":
        delete_callback(
            callback_context.bot,
            update.callback_query.message.chat.id,
            data["bot_name"],
        )
    elif data["action"] == "add":
        if "day" in data:
            choose_slot_callback(
                callback_context.bot, update.callback_query.message.chat.id, data["day"]
            )
        elif "bot_name" in data:
            add_bot_callback(
                callback_context.bot,
                update.callback_query.message.chat.id,
                data["bot_name"],
            )


callback_query_handler = CallbackQueryHandler(callback_manager)
dispatcher.add_handler(callback_query_handler)


def help(update, context):
    logger.info("chat_id={}".format(update.message.chat_id))
    help_text = """
    accept - accept court(s) attending
    add - create a bot for day of week at specific slot time 
    bookings - display all bookings  
    bots - display all bots and their statuses
    cancel - cancel a booking
    delete - delete a bot  
    deploy - start all the created bots  
    help - display this message
    """
    context.bot.send_message(chat_id=update.message.chat_id, text=help_text)


help_handler = CommandHandler("help", help)
dispatcher.add_handler(help_handler)


def unknown(update, context):
    context.bot.send_message(
        chat_id=update.message.chat_id, text="Sorry, I didn't understand that command."
    )


unknown_handler = MessageHandler(Filters.command, unknown)
dispatcher.add_handler(unknown_handler)

updater.start_polling()
