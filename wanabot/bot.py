import json

import config
from telegram.ext import Updater, Handler, CommandHandler, MessageHandler, Filters
import logging
import requests

logging.basicConfig(format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
                     level=logging.INFO)
logger = logging.getLogger(__name__)
updater = Updater(token=config.token, use_context=True)
dispatcher = updater.dispatcher

def get_bookings():
    response = requests.get('{}/bookings'.format(config.booker_api))
    bookings = json.loads(response.content)
    print(bookings)
    return bookings

def bookings(update, context):
    bookings = get_bookings()
    if len(bookings) == 0:
        text = 'no bookings found'
    else:
        text = ''
        cpt = 0
        for booking in bookings:
            cpt +=1
            text += '({}) {} at {} on court {}\n'.format(cpt, booking['date'], booking['court_time'],
                                                             booking['court_number'])

    context.bot.send_message(chat_id=update.message.chat_id, text=text)

bookings_handler = CommandHandler('bookings', bookings)
dispatcher.add_handler(bookings_handler)

def bots(update, context):
    response = requests.get('{}/bots'.format(config.booker_api))
    bots = json.loads(response.content)
    if len(bots) == 0:
        text = 'no bots found'
    else:
        text = ''
        for bot in bots:
            text += '{}: {} at {} ({})\n'.format(bot['name'], bot['week_day'], bot['court_time'], bot['status'])

    context.bot.send_message(chat_id=update.message.chat_id, text=text)

bots_handler = CommandHandler('bots', bots)
dispatcher.add_handler(bots_handler)

def handle_response(bot, response, chat_id, usage):
    print('response code = {}'.format(response.status_code))
    status = 'ok' if response.status_code >= 200 and response.status_code < 300 else 'ko, usage: \n{}'.format(usage)
    bot.send_message(chat_id=chat_id, text=status)


def deploy(update, context):
    usage = '/deploy'
    response = requests.post('{}/bots/actions/deploy'.format(config.booker_api))
    handle_response(context.bot, response, update.message.chat_id, usage)

deploy_handler = CommandHandler('deploy', deploy)
dispatcher.add_handler(deploy_handler)


def add(update, context):
    usage = '/add [day] [court_time] (ex: /add monday 19:20)'
    logger.info(context.args)
    if len(context.args) != 2:
        context.bot.send_message(chat_id=update.message.chat_id, text='ko: usage: \n{}'.format(usage))
        return
    day = context.args[0]
    court_time = context.args[1]
    payload = {
        'name': 'bot_{}_{}'.format(day, court_time).replace(':', '_'),
        'week_day': day[0].upper() + day[1:],
        'court_time': court_time,
        'status': 'Created',
    }
    print(payload)
    response = requests.post('{}/bots'.format(config.booker_api), json=payload)
    handle_response(context.bot, response, update.message.chat_id, usage)

add_handler = CommandHandler('add', add)
dispatcher.add_handler(add_handler)

def delete(update, context):
    usage = '/delete [bot_name] (ex: /delete bot_monday_19_20)'
    logger.info(context.args)
    if len(context.args) != 1:
        context.bot.send_message(chat_id=update.message.chat_id, text='ko: usage: \n{}'.format(usage))
        return
    bot = context.args[0]
    response = requests.delete('{}/bots/{}'.format(config.booker_api, bot))
    handle_response(context.bot, response, update.message.chat_id, usage)

delete_handler = CommandHandler('delete', delete)
dispatcher.add_handler(delete_handler)

def cancel(update, context):
    usage = '/cancel [booking_number] (ex: /cancel 1)'
    logger.info(context.args)
    if len(context.args) != 1:
        context.bot.send_message(chat_id=update.message.chat_id, text='ko: usage: \n{}'.format(usage))
        return
    idx = context.args[0]
    bookings = get_bookings()
    booking_id = bookings[int(idx) - 1]['id']
    response = requests.delete('{}/bookings/{}'.format(config.booker_api, booking_id))
    handle_response(context.bot, response, update.message.chat_id, usage)

cancel_handler = CommandHandler('cancel', cancel)
dispatcher.add_handler(cancel_handler)

def echo(update, context):
    logger.info('echo "{}"'.format(update.message.text))
    context.bot.send_message(chat_id=update.message.chat_id, text=update.message.text)

echo_handler = MessageHandler(Filters.text, echo)
dispatcher.add_handler(echo_handler)

def help(update, context):
    help_text = """
    commands availables:
    /add [day] [court_time] -> create a bot for day of week [day] at [court_time]  
    /bookings -> display all bookings  
    /bots -> display all bots and their statuses
    /cancel [booking_number] -> cancel the booking #[booking_number]
    /delete [bot_name] -> delete the bot [bot_name]  
    /deploy -> start all the created bots  
    /help -> display this message
    """
    context.bot.send_message(chat_id=update.message.chat_id, text=help_text)

help_handler = CommandHandler('help', help)
dispatcher.add_handler(help_handler)

def unknown(update, context):
    context.bot.send_message(chat_id=update.message.chat_id, text="Sorry, I didn't understand that command.")

unknown_handler = MessageHandler(Filters.command, unknown)
dispatcher.add_handler(unknown_handler)

updater.start_polling()